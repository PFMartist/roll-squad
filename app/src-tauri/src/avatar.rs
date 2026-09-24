// 头像：本地缓存 → 联网取 → 落盘。对应 tools/roll_web.py 的头像那段。
//
// 图片 URL 不能靠拼：media.prts.wiki 的路径带哈希目录，只能先问 api.php。
// 索引与图片都缓存在 data\ 下，**离线时命中缓存就完全不联网**。

use crate::paths;
use crate::roster::http_client;
use std::collections::HashMap;
use std::time::Duration;

const BATCH: usize = 30;

pub fn load_index() -> HashMap<String, String> {
    std::fs::read_to_string(paths::avatars_index_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_index(map: &HashMap<String, String>) {
    if map.is_empty() {
        return;
    }
    let _ = paths::ensure_dir(&paths::data_dir());
    if let Ok(t) = serde_json::to_string(map) {
        let _ = std::fs::write(paths::avatars_index_path(), t);
    }
}

/// 把还没解析过的名字批量问 api.php（一次 30 个），结果并进索引。
/// 网络失败**不写索引**（否则会把"取不到"永久记成"这人没头像"）。
pub fn resolve_names(names: &[String]) -> HashMap<String, String> {
    let mut idx = load_index();
    let todo: Vec<String> = names
        .iter()
        .filter(|n| !n.is_empty() && !idx.contains_key(*n) && !n.contains('\u{FFFD}'))
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    if todo.is_empty() {
        return idx;
    }
    let Ok(client) = http_client() else { return idx };

    let mut changed = false;
    for chunk in todo.chunks(BATCH) {
        let titles = chunk
            .iter()
            .map(|n| format!("File:头像_{n}.png"))
            .collect::<Vec<_>>()
            .join("|");
        let resp = client
            .get("https://prts.wiki/api.php")
            .query(&[
                ("action", "query"),
                ("titles", titles.as_str()),
                ("prop", "imageinfo"),
                ("iiprop", "url"),
                ("format", "json"),
            ])
            .send();
        let Ok(resp) = resp else { continue }; // 没网：跳过，不写索引
        let Ok(v) = resp.json::<serde_json::Value>() else { continue };
        let Some(pages) = v.pointer("/query/pages").and_then(|x| x.as_object()) else { continue };
        let mut got: HashMap<String, String> = HashMap::new();
        for page in pages.values() {
            let title = page.get("title").and_then(|x| x.as_str()).unwrap_or("");
            let name = title
                .trim_start_matches("文件:头像 ")
                .trim_end_matches(".png")
                .to_string();
            let url = page
                .pointer("/imageinfo/0/url")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            got.insert(name, url);
        }
        for n in chunk {
            // 明确"缺失"记空串（负缓存，免得反复问）；取不到的名字根本不在 got 里 → 记空但可重试
            idx.insert(n.clone(), got.get(n).cloned().unwrap_or_default());
        }
        changed = true;
    }
    if changed {
        save_index(&idx);
    }
    idx
}

/// 取一张头像的字节：缓存命中直接读盘，否则下载并落盘。失败返回 None（前端退化色块）。
pub fn avatar_bytes(name: &str) -> Option<Vec<u8>> {
    if name.is_empty() || name.contains('\u{FFFD}') {
        return None;
    }
    let file = paths::avatar_file(name);
    if let Ok(b) = std::fs::read(&file) {
        return Some(b);
    }
    let idx = resolve_names(&[name.to_string()]);
    let url = idx.get(name)?;
    if url.is_empty() {
        return None;
    }
    let client = http_client().ok()?;
    let resp = client.get(url).timeout(Duration::from_secs(20)).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().ok()?.to_vec();
    let _ = paths::ensure_dir(&paths::avatars_dir());
    let _ = std::fs::write(&file, &bytes);
    Some(bytes)
}

/// 全量预热：把名字列表里的头像都抓进缓存，返回成功张数。
/// 打包时传的是**全游戏干员**（职业索引的键 ∪ 你 box 里的名字），而不是只有你拥有的——
/// 否则别人拿到这份包、或你抽到没拥有的干员时就没脸。
pub fn prefetch(names: &[String]) -> usize {
    let _ = resolve_names(names);
    let mut ok = 0;
    for n in names {
        if paths::avatar_file(n).is_file() {
            ok += 1;
            continue;
        }
        if avatar_bytes(n).is_some() {
            ok += 1;
        }
        std::thread::sleep(Duration::from_millis(120)); // 别把人家打疼
    }
    ok
}

/// 缓存里现在有多少张（打包脚本用来核对"是否全量"）
pub fn cached_count() -> usize {
    std::fs::read_dir(paths::avatars_dir())
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false)).count())
        .unwrap_or(0)
}
