// 前端能调的三个命令，形状与 Python 版的 /api/state、/api/roll、/api/settings 一一对应。

use crate::model::{Config, Operator};
use crate::{avatar, paths, roll, roster};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// 联网取头像的总开关（界面上的复选框控制）
pub static AVATARS_ENABLED: AtomicBool = AtomicBool::new(true);

// ---------------------------------------------------------------- 配置

pub fn load_config() -> Config {
    std::fs::read_to_string(paths::config_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_config(cfg: &Config) -> Result<(), String> {
    let _ = paths::ensure_dir(&paths::data_dir());
    let t = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(paths::config_path(), t).map_err(|e| format!("写配置失败：{e}"))
}

/// 当前用哪份 box：配置指定的（相对 data 解析），没配就取 data 下名字最大的 box*.json
pub fn current_box(cfg: &Config) -> Option<PathBuf> {
    let data = paths::data_dir();
    if !cfg.box_file.is_empty() {
        let p = PathBuf::from(&cfg.box_file);
        let p = if p.is_absolute() { p } else { data.join(&cfg.box_file) };
        if p.is_file() {
            return Some(p);
        }
    }
    let mut cands: Vec<PathBuf> = std::fs::read_dir(&data)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .map(|n| {
                        let n = n.to_string_lossy().to_lowercase();
                        n.starts_with("box") && n.ends_with(".json")
                    })
                    .unwrap_or(false)
        })
        .collect();
    cands.sort();
    cands.pop()
}

fn default_roll_defaults() -> Value {
    json!({
        "tier": 0, "mode": "quota", "n": 12, "avoid_last": 1, "exclude": "",
        "rarities": [1, 2, 3, 4, 5, 6],
        // 八个职业各一名（与 roll::default_floor 一致）
        "quota": { "先锋": 1, "近卫": 1, "重装": 1, "狙击": 1, "术师": 1, "医疗": 1, "辅助": 1, "特种": 1 }
    })
}

/// 内置默认 ← config 里存的那套（缺的键用内置值补齐）
fn roll_defaults(cfg: &Config) -> Value {
    let mut base = default_roll_defaults();
    if let (Some(b), Some(o)) = (base.as_object_mut(), cfg.roll_defaults.as_object()) {
        for (k, v) in o {
            if b.contains_key(k) {
                b.insert(k.clone(), v.clone());
            }
        }
    }
    base
}

/// 把本次抽签用的参数记成下次的默认值（seed 故意不记 —— 记了就永远是同一队）
fn merge_defaults(cfg: &Config, body: &Value) -> Value {
    let mut d = roll_defaults(cfg);
    let Some(dm) = d.as_object_mut() else { return d };
    if let Some(v) = body.get("tier").and_then(|x| x.as_u64()) {
        dm.insert("tier".into(), json!(v));
    }
    if let Some(v) = body.get("mode").and_then(|x| x.as_str()) {
        dm.insert("mode".into(), json!(if v == "pure" { "pure" } else { "quota" }));
    }
    if let Some(v) = body.get("n").and_then(|x| x.as_u64()) {
        dm.insert("n".into(), json!(v));
    }
    if let Some(v) = body.get("avoid_last").and_then(|x| x.as_u64()) {
        dm.insert("avoid_last".into(), json!(v));
    }
    if let Some(arr) = body.get("exclude").and_then(|x| x.as_array()) {
        let s = arr.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ");
        dm.insert("exclude".into(), json!(s));
    }
    if let Some(arr) = body.get("rarities").and_then(|x| x.as_array()) {
        dm.insert("rarities".into(), Value::Array(arr.clone()));
    }
    if let Some(q) = body.get("quota") {
        if q.is_object() {
            dm.insert("quota".into(), q.clone());
        }
    }
    d
}

// ---------------------------------------------------------------- box 列表

fn box_options() -> Vec<Value> {
    let data = paths::data_dir();
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&data) else { return out };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .map(|n| {
                        let n = n.to_string_lossy().to_lowercase();
                        n.starts_with("box") && n.ends_with(".json")
                    })
                    .unwrap_or(false)
        })
        .collect();
    files.sort();
    files.reverse(); // 新的在前
    for p in files {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let size_kb = std::fs::metadata(&p).map(|m| m.len() / 1024).unwrap_or(0);
        out.push(json!({
            "file": name, "name": name, "size_kb": size_kb,
            "sync": roster::box_sync_text(&p),
        }));
    }
    out
}

// ---------------------------------------------------------------- 三个命令

#[tauri::command]
pub fn get_state() -> Result<Value, String> {
    let cfg = load_config();
    AVATARS_ENABLED.store(cfg.fetch_avatars, Ordering::Relaxed);

    let box_path = current_box(&cfg);
    let ops: Vec<Operator> = match &box_path {
        Some(p) => roster::build_roster(p).unwrap_or_default(),
        None => Vec::new(),
    };

    let (box_file, sync, age) = match &box_path {
        Some(p) => (
            p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            roster::box_sync_text(p),
            roster::box_age_days(p),
        ),
        None => (String::new(), None, None),
    };

    // 档位编号即包含关系（0 最宽 → 5 最严），顺序遍历即可
    let mut tiers = Map::new();
    for t in 0..=5u32 {
        let pool: Vec<Operator> = ops.iter().filter(|o| roster::tier_pass(o, t)).cloned().collect();
        tiers.insert(
            t.to_string(),
            json!({ "n": pool.len(), "desc": roster::tier_desc(t), "prof": roster::profession_hist(&pool) }),
        );
    }

    let history = roll::history_for_ui(&roll::load_history(), 8);

    Ok(json!({
        "roster": ops.len(),
        "box": {
            "file": box_file, "path": box_file, "sync": sync,
            "age_days": age, "stale": age.map(|d| d > 14).unwrap_or(false),
        },
        "box_options": box_options(),
        "avatars_enabled": cfg.fetch_avatars,
        "tiers": Value::Object(tiers),
        "professions": roster::PROFESSION_ORDER,
        "floor": roll::default_floor(),
        "history": history,
        "defaults": roll_defaults(&cfg),
        "modes": { "floor": "保底", "pure": "纯随机", "quota": "配额" },
    }))
}

fn parse_req(body: &Value) -> roll::RollReq {
    let get_u32 = |k: &str| body.get(k).and_then(|x| x.as_u64()).map(|v| v as u32);
    roll::RollReq {
        tier: get_u32("tier").unwrap_or(0),
        mode: body.get("mode").and_then(|x| x.as_str()).unwrap_or("quota").to_string(),
        quota: body.get("quota").and_then(|q| {
            q.as_object().map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n as u32)))
                    .collect::<HashMap<String, u32>>()
            })
        }),
        n: get_u32("n").unwrap_or(12).max(1) as usize,
        seed: body.get("seed").and_then(|x| x.as_u64()),
        avoid_last: get_u32("avoid_last").unwrap_or(0),
        exclude: body
            .get("exclude")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default(),
        rarities: body
            .get("rarities")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_u64().map(|v| v as u32)).collect()),
        min_elite: get_u32("min_elite"),
        min_level: get_u32("min_level"),
        max_level: get_u32("max_level"),
        min_rarity: get_u32("min_rarity"),
        slots: body.get("slots").and_then(|x| x.as_array()).map(|a| {
            a.iter()
                .map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<Option<String>>>()
        }),
    }
}

#[tauri::command]
pub fn roll(body: Value) -> Result<Value, String> {
    let cfg = load_config();
    let box_path = current_box(&cfg).ok_or_else(|| {
        format!(
            "还没有干员池文件。把 MAA 导出的 OperBoxData.json 放进：\n{}",
            paths::data_dir().display()
        )
    })?;
    let ops = roster::build_roster(&box_path)?;
    let req = parse_req(&body);
    let out = roll::build_and_roll(&ops, &req)?;

    // 抽成功就把这套参数记成下次的默认值
    let mut cfg2 = cfg.clone();
    cfg2.roll_defaults = merge_defaults(&cfg, &body);
    let _ = save_config(&cfg2);

    Ok(json!({
        "squad": out.squad,
        "seed": out.seed,
        "mode": if req.mode == "pure" { "pure" } else { "quota" },
        "tier": req.tier,
        "n": req.n,
        "quota": req.quota,
        "pool": {
            "roster": out.roster_n,
            "after_tier": out.after_tier,
            "after_filter": out.after_filter,
        },
        "warnings": out.warnings,
        "history_file": paths::history_path().display().to_string(),
        "session": Value::Null,
    }))
}

#[tauri::command]
pub fn save_settings(body: Value) -> Result<Value, String> {
    let mut cfg = load_config();
    if let Some(b) = body.get("avatars").and_then(|x| x.as_bool()) {
        cfg.fetch_avatars = b;
    }
    if let Some(name) = body.get("box").and_then(|x| x.as_str()) {
        if !name.is_empty() {
            let p = paths::data_dir().join(name);
            if !p.is_file() {
                return Err(format!("没有这份干员池：{}", p.display()));
            }
            let ops = roster::build_roster(&p)?;
            if ops.is_empty() {
                return Err(format!("这份干员池里一个干员都没有：{name}"));
            }
            cfg.box_file = name.to_string();
        }
    }
    save_config(&cfg)?;
    AVATARS_ENABLED.store(cfg.fetch_avatars, Ordering::Relaxed);
    get_state()
}

/// 全量头像预热（全游戏干员，不只是你有的那些）——打包前跑，也供界面手动触发
#[tauri::command]
pub fn prefetch_avatars() -> Result<serde_json::Value, String> {
    let names = crate::all_operator_names();
    if names.is_empty() {
        return Err("既没有职业索引也没有干员池文件".into());
    }
    let ok = avatar::prefetch(&names);
    Ok(json!({ "cached": ok, "total": names.len() }))
}
