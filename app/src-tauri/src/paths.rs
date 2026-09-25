// 数据目录定位。
//
// 便携形态：exe 同级的 data\（整个文件夹拷到哪都能跑）。
// 开发时（debug 构建）落到 app/devdata\，否则会写进 target\debug\ 里找不到数据。
// 任何情况下都可用环境变量 ROLL_SQUAD_DATA 覆盖（测试用）。
// 安卓没有"exe 同级目录"，由 lib.rs 的 setup 调 set_data_dir 指到应用私有目录。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// 只在启动时调一次（安卓：应用私有目录）。设过之后 data_dir() 一律返回它。
pub fn set_data_dir(p: PathBuf) {
    let _ = OVERRIDE.set(p);
}

pub fn data_dir() -> PathBuf {
    if let Some(p) = OVERRIDE.get() {
        return p.clone();
    }
    if let Ok(p) = std::env::var("ROLL_SQUAD_DATA") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("devdata");
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("data")))
        .unwrap_or_else(|| PathBuf::from("data"))
}

pub fn ensure_dir(p: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(p)
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn history_path() -> PathBuf {
    data_dir().join("history.json")
}

pub fn professions_path() -> PathBuf {
    data_dir().join("prts_professions.json")
}

pub fn char_table_path() -> PathBuf {
    data_dir().join("character_table.json")
}

pub fn avatars_index_path() -> PathBuf {
    data_dir().join("avatars.json")
}

pub fn avatars_dir() -> PathBuf {
    data_dir().join("avatars")
}

/// 干员名 → 头像缓存文件名。名字里有括号、间隔号，但 Windows 非法字符要换掉。
pub fn avatar_file(name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    avatars_dir().join(format!("{safe}.png"))
}
