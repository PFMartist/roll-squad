// 随机编队 · 桌面版
//
// 前端就是仓库根的 web\（通过 tauri.conf.json 的 frontendDist 编进 exe），
// 浏览器版与桌面版共用同一份前端源码，靠 app.js 里的宿主垫片分流。

mod avatar;
mod commands;
mod model;
mod paths;
mod roll;
mod roster;

use std::sync::atomic::Ordering;
use tauri::http::Response;
use tauri::Manager;

/// 头像通过自定义协议供给前端。
/// Windows 上前端拿到的 URL 形如 http://avatar.localhost/<percent-encoded 名字>，
/// 由 app.js 的 convertFileSrc 生成，这里负责解码成干员名。
fn avatar_response(name: &str) -> Response<Vec<u8>> {
    let not_found = || Response::builder().status(404).body(Vec::new()).unwrap();
    if !commands::AVATARS_ENABLED.load(Ordering::Relaxed) {
        return not_found(); // 关掉开关 = 一个出网请求都不发
    }
    match avatar::avatar_bytes(name) {
        Some(bytes) => Response::builder()
            .status(200)
            .header("Content-Type", "image/png")
            .body(bytes)
            .unwrap(),
        None => not_found(),
    }
}

/// 给 main.rs 写日志用（paths 是私有模块）
pub fn data_dir_for_log() -> std::path::PathBuf {
    paths::data_dir()
}

/// 全游戏干员名列表：职业索引的键 ∪ box 里的名字（索引可能缺人，box 一定准）
pub fn all_operator_names() -> Vec<String> {
    let cfg = commands::load_config();
    let mut names: Vec<String> = roster::load_professions().keys().cloned().collect();
    if let Some(p) = commands::current_box(&cfg) {
        if let Ok(ops) = roster::build_roster(&p) {
            names.extend(ops.into_iter().map(|o| o.name));
        }
    }
    names.retain(|n| !n.is_empty());
    names.sort();
    names.dedup();
    names
}

/// 打包前的头像预热（无界面模式，由 --prefetch-avatars 触发）
pub fn run_prefetch() -> Result<usize, String> {
    let names = all_operator_names();
    if names.is_empty() {
        return Err("既没有职业索引也没有干员池文件（data\\box*.json）".into());
    }
    Ok(avatar::prefetch(&names))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())   // 系统文件选择器（导入 box）
        .plugin(tauri_plugin_fs::init())       // 读选择器给的文件：安卓上是 content:// 需要用它的原生实现
        .setup(|app| {
            // 安卓上 current_exe() 指向 /system/bin/app_process，exe 同级目录没有意义，
            // 数据改放应用私有目录（卸载即清）。桌面版保持便携形态，不动。
            #[cfg(mobile)]
            {
                let dir = app.path().app_data_dir()?;
                paths::set_data_dir(dir);
            }
            paths::ensure_dir(&paths::data_dir())?;
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("avatar", |_ctx, req, responder| {
            let path = req.uri().path().to_string();
            std::thread::spawn(move || {
                let name = percent_encoding::percent_decode_str(path.trim_start_matches('/'))
                    .decode_utf8_lossy()
                    .to_string();
                responder.respond(avatar_response(&name));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::roll,
            commands::save_settings,
            commands::import_box,
            commands::prefetch_avatars,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
