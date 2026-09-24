// 干员池：box（谁 + 练度 + 星级）× 职业。
//
// 职业的来源按优先级：本地主表（可选，放了才用）→ PRTS 职业索引（11 KB，随包预置）
// → 都没有就记「未知」**但保留在池子里**（Python 版早期会剔除，这里不重蹈覆辙）。
// 对应 tools/roster.py。

use crate::model::{BoxOp, Operator};
use crate::paths;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub const PROFESSION_ORDER: [&str; 8] = ["先锋", "近卫", "重装", "狙击", "术师", "医疗", "辅助", "特种"];
pub const UNKNOWN_PROF: &str = "未知";

const PROFESSION_CN: [(&str, &str); 8] = [
    ("PIONEER", "先锋"), ("WARRIOR", "近卫"), ("TANK", "重装"), ("SNIPER", "狙击"),
    ("CASTER", "术师"), ("MEDIC", "医疗"), ("SUPPORT", "辅助"), ("SPECIAL", "特种"),
];
const SKIP_PROFESSIONS: [&str; 2] = ["TOKEN", "TRAP"];

/// 别用浏览器式 UA：prts.wiki 的 WAF 会 403（实测）。自定义 UA 反而放行。
const PRTS_UA: &str = "maa-roll-squad/1.0 (personal tool)";

/// 档位 → (最低精英化, 最低等级)。与 Python 版 TIER_RULE 一致。
pub fn tier_gate(tier: u32) -> (u32, u32) {
    match tier {
        1 => (1, 0),
        2 => (2, 0),
        3 => (2, 80),
        _ => (0, 0),
    }
}

pub fn tier_desc(tier: u32) -> &'static str {
    match tier {
        1 => "精一及以上",
        2 => "精二",
        3 => "精二且 Lv≥80",
        _ => "全部持有",
    }
}

pub fn http_client() -> Result<reqwest::blocking::Client, String> {
    let mut b = reqwest::blocking::Client::builder()
        .user_agent(PRTS_UA)
        .timeout(Duration::from_secs(25));
    // 系统代理（本机 Clash 在 7890）——不设也能跑，设了更稳
    for var in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY"] {
        if let Ok(p) = std::env::var(var) {
            if !p.is_empty() {
                if let Ok(proxy) = reqwest::Proxy::all(&p) {
                    b = b.proxy(proxy);
                }
                break;
            }
        }
    }
    b.build().map_err(|e| format!("建 HTTP 客户端失败：{e}"))
}

// ---------------------------------------------------------------- box

pub fn load_box(path: &Path) -> Result<Vec<BoxOp>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读不了 {}\n{e}", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} 不是合法 JSON：{e}", path.display()))?;
    let arr = if let Some(a) = v.as_array() {
        a.clone()
    } else {
        let mut found = None;
        for key in ["own_opers", "chars", "opers", "data"] {
            if let Some(a) = v.get(key).and_then(|x| x.as_array()) {
                found = Some(a.clone());
                break;
            }
        }
        found.ok_or_else(|| "box 里找不到干员数组（own_opers / chars / opers / data）".to_string())?
    };
    let ops: Vec<BoxOp> = arr
        .iter()
        .filter_map(|x| serde_json::from_value::<BoxOp>(x.clone()).ok())
        .filter(|o| !o.name.is_empty())
        .collect();
    if ops.is_empty() {
        return Err(format!("{} 里一个干员都没有", path.display()));
    }
    Ok(ops)
}

/// box 的同步时间（MAA 导出的 syncTime，形如 2026-09-12T00:17:10.7447143+08:00）
pub fn box_sync_text(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let s = v.get("syncTime")?.as_str()?;
    Some(s.chars().take(10).collect())
}

/// 距今多少天（只按日期算，不涉及时区秒级精度）。取不到返回 None。
pub fn box_age_days(path: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let s = v.get("syncTime")?.as_str()?;
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let then = days_from_civil(y, m, d);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64
        / 86400;
    Some(now - then)
}

/// Howard Hinnant 的民用历→天数（避免为一个日期计算拉 chrono 进来）
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

// ---------------------------------------------------------------- 职业来源

/// 本地干员主表（可选）：id → (名字, 中文职业, 分支)
struct CharTable {
    by_id: HashMap<String, (String, String, Option<String>)>,
    by_name: HashMap<String, (String, String, Option<String>)>,
}

fn load_char_table() -> Option<CharTable> {
    let path = paths::char_table_path();
    if !path.is_file() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let obj = v.as_object()?;
    let mut by_id = HashMap::new();
    let mut by_name = HashMap::new();
    for (id, rec) in obj {
        let prof_en = rec.get("profession").and_then(|x| x.as_str()).unwrap_or("");
        if SKIP_PROFESSIONS.contains(&prof_en) {
            continue;
        }
        if rec.get("isNotObtainable").and_then(|x| x.as_bool()).unwrap_or(false) {
            continue;
        }
        let name = rec.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let prof = PROFESSION_CN
            .iter()
            .find(|(en, _)| *en == prof_en)
            .map(|(_, cn)| cn.to_string())
            .unwrap_or_else(|| prof_en.to_string());
        let sub = rec.get("subProfessionId").and_then(|x| x.as_str()).map(|s| s.to_string());
        let entry = (name.clone(), prof, sub);
        by_id.insert(id.clone(), entry.clone());
        by_name.insert(name, entry);
    }
    Some(CharTable { by_id, by_name })
}

/// PRTS 职业索引：干员名 → 职业
pub fn load_professions() -> HashMap<String, String> {
    let path = paths::professions_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_professions(map: &HashMap<String, String>) {
    if map.is_empty() {
        return;
    }
    let _ = paths::ensure_dir(&paths::data_dir());
    if let Ok(t) = serde_json::to_string(map) {
        let _ = std::fs::write(paths::professions_path(), t);
    }
}

/// 从 PRTS 的八个职业分类抓全量「干员名 → 职业」。失败返回 Err（调用方退回缓存）。
pub fn fetch_professions() -> Result<HashMap<String, String>, String> {
    let client = http_client()?;
    let mut out = HashMap::new();
    for (_, cn) in PROFESSION_CN {
        let cat = format!("Category:{cn}干员");
        let mut cont: Option<String> = None;
        loop {
            let mut req = client
                .get("https://prts.wiki/api.php")
                .query(&[
                    ("action", "query"),
                    ("list", "categorymembers"),
                    ("cmtitle", cat.as_str()),
                    ("cmlimit", "500"),
                    ("cmnamespace", "0"),
                    ("format", "json"),
                ]);
            if let Some(c) = cont.as_deref() {
                req = req.query(&[("cmcontinue", c)]);
            }
            let v: serde_json::Value = req
                .send()
                .map_err(|e| format!("请求 {cat} 失败：{e}"))?
                .json()
                .map_err(|e| format!("{cat} 返回的不是 JSON：{e}"))?;
            if let Some(items) = v.pointer("/query/categorymembers").and_then(|x| x.as_array()) {
                for it in items {
                    if let Some(t) = it.get("title").and_then(|x| x.as_str()) {
                        out.insert(t.to_string(), cn.to_string());
                    }
                }
            }
            cont = v.pointer("/continue/cmcontinue").and_then(|x| x.as_str()).map(|s| s.to_string());
            if cont.is_none() {
                break;
            }
        }
    }
    if out.is_empty() {
        return Err("PRTS 一个干员都没抓到".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- 组装

/// 全游戏唯一能转职的干员。她游戏里可在术师/近卫/医疗之间切，
/// 所以配额凑不够时允许她顶上（见 roll.rs 的候选人筛选）。
const AMIYA_ID: &str = "char_002_amiya";
const AMIYA_NAME: &str = "阿米娅";
const AMIYA_ALT: [&str; 2] = ["近卫", "医疗"]; // 她的基础职业是术师

fn make_op(b: &BoxOp, profession: String, sub: Option<String>) -> Operator {
    let alt = if b.id == AMIYA_ID || b.name == AMIYA_NAME {
        AMIYA_ALT.iter().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };
    Operator {
        id: if b.id.is_empty() { b.name.clone() } else { b.id.clone() },
        name: b.name.clone(),
        rarity: b.rarity,
        elite: b.elite,
        level: b.level,
        potential: b.potential,
        own: true,
        profession,
        sub_profession: sub,
        skill_level: None,
        module: None,
        alt_professions: alt,
        converted: false,
    }
}

/// box + 职业 → 统一形状的干员池。**任何情况下都不剔除干员**。
pub fn build_roster(box_path: &Path) -> Result<Vec<Operator>, String> {
    let box_ops = load_box(box_path)?;
    let table = load_char_table();

    let mut out = Vec::with_capacity(box_ops.len());
    let mut need: Vec<BoxOp> = Vec::new();
    for b in box_ops {
        match table.as_ref().and_then(|t| {
            t.by_id
                .get(&b.id)
                .or_else(|| t.by_name.get(&b.name))
                .map(|(_, prof, sub)| (prof.clone(), sub.clone()))
        }) {
            Some((prof, sub)) => out.push(make_op(&b, prof, sub)),
            None => need.push(b),
        }
    }

    if !need.is_empty() {
        let mut idx = load_professions();
        let missing: Vec<String> = need
            .iter()
            .map(|b| b.name.clone())
            .filter(|n| !idx.contains_key(n))
            .collect();
        if !missing.is_empty() {
            // 可能是新干员：联网刷新一次索引（离线就退回缓存，不阻塞）
            if let Ok(fresh) = fetch_professions() {
                save_professions(&fresh);
                idx = fresh;
            }
        }
        for b in need {
            let prof = idx.get(&b.name).cloned().unwrap_or_else(|| UNKNOWN_PROF.to_string());
            out.push(make_op(&b, prof, None));
        }
    }
    Ok(out)
}

/// 按档位 + 可选门槛筛
pub fn apply_tier(ops: &[Operator], tier: u32, min_elite: u32, min_level: u32) -> Vec<Operator> {
    let (e, l) = tier_gate(tier);
    let (e, l) = (e.max(min_elite), l.max(min_level));
    ops.iter()
        .filter(|o| o.elite >= e && o.level >= l)
        .cloned()
        .collect()
}

/// 每个档位各有多少人（给前端下拉框显示）
pub fn tier_counts(ops: &[Operator]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for tier in 0..=3u32 {
        let (e, l) = tier_gate(tier);
        let n = ops.iter().filter(|o| o.elite >= e && o.level >= l).count();
        m.insert(tier.to_string(), serde_json::json!(n));
    }
    serde_json::Value::Object(m)
}

/// 某个池子的职业分布
pub fn profession_hist(ops: &[Operator]) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for p in PROFESSION_ORDER {
        let n = ops.iter().filter(|o| o.profession == p).count();
        if n > 0 {
            m.insert(p.to_string(), serde_json::json!(n));
        }
    }
    serde_json::Value::Object(m)
}
