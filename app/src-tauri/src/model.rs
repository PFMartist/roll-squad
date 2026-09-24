// 数据结构。字段名**对齐现有 JSON 文件**，这样 Python 版写出来的 box / 历史 / 配置
// 可以直接被桌面版读，反之亦然。

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- box（MAA 干员识别导出）

#[derive(Debug, Clone, Deserialize)]
pub struct BoxOp {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub rarity: u32,
    #[serde(default)]
    pub elite: u32,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub potential: u32,
    #[serde(default)]
    pub own: bool,
}

// ---------------------------------------------------------------- 统一形状（前端认这个）

#[derive(Debug, Clone, Serialize)]
pub struct Operator {
    pub id: String,
    pub name: String,
    pub rarity: u32,
    pub elite: u32,
    pub level: u32,
    pub potential: u32,
    pub own: bool,
    pub profession: String,
    pub sub_profession: Option<String>,
    pub skill_level: Option<u32>, // MAA 读不到 → None（接森空岛后才有）
    pub module: Option<u32>,
    /// 可以转职顶上的职业（目前只有阿米娅：术师/近卫/医疗）
    #[serde(default)]
    pub alt_professions: Vec<String>,
    /// 本局是按转职后的职业出战的（前端据此标"转职"）
    #[serde(default)]
    pub converted: bool,
}

// ---------------------------------------------------------------- 配置

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_box_file")]
    pub box_file: String,
    #[serde(default = "default_true")]
    pub fetch_avatars: bool,
    #[serde(default)]
    pub roll_defaults: serde_json::Value,
}

fn default_box_file() -> String {
    String::new()
}
fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Config {
            box_file: String::new(),
            fetch_avatars: true,
            roll_defaults: serde_json::json!({}),
        }
    }
}

// ---------------------------------------------------------------- 历史

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMember {
    pub id: String,
    pub name: String,
    pub profession: String,
    pub rarity: u32,
    pub elite: u32,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub time: String,
    pub seed: u64,
    pub mode: String,
    pub tier: u32,
    pub n: u32,
    #[serde(default)]
    pub quota: Option<serde_json::Value>,
    #[serde(default)]
    pub pool: serde_json::Value,
    #[serde(default)]
    pub members: Vec<HistoryMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct History {
    #[serde(default)]
    pub rolls: Vec<HistoryEntry>,
}
