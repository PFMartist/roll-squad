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

// ---------------------------------------------------------------- 读文本

/// 解码一段文本字节，**容忍开头的 BOM**。
///
/// 为什么必须容忍：serde_json 不跳 BOM，带 BOM 的文件会在第一个字节就报
/// `expected value at line 1 column 1` —— 用户看到的是"文件明明是好的却说不是合法 JSON"。
/// 而 Windows 生态里给 UTF-8 加 BOM 是常见默认（记事本的"UTF-8"另存、
/// PowerShell 5.1 的 `Set-Content -Encoding UTF8`），所以这条路上一定会有人踩。
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned(); // UTF-8 BOM：本体还是 UTF-8，丢掉前缀即可
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16_to_string(rest, true); // UTF-16 小端
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16_to_string(rest, false); // UTF-16 大端
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn utf16_to_string(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| if little_endian { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
        .collect();
    String::from_utf16_lossy(&units)
}

/// 读文本文件，容忍 BOM 与 UTF-16。所有 JSON 都该走这里。
pub fn read_text(p: &Path) -> std::io::Result<String> {
    Ok(decode_text(&std::fs::read(p)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = "{\"done\":true,\"own_opers\":[]}";

    fn utf16_bytes(little_endian: bool) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(if little_endian { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
        for u in JSON.encode_utf16() {
            let pair = if little_endian { u.to_le_bytes() } else { u.to_be_bytes() };
            v.extend_from_slice(&pair);
        }
        v
    }

    #[test]
    fn plain_utf8_passes_through() {
        assert_eq!(decode_text(JSON.as_bytes()), JSON);
    }

    #[test]
    fn utf8_bom_is_stripped() {
        // 就是这条让用户报 "expected value at line 1 column 1"
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(JSON.as_bytes());
        let text = decode_text(&bytes);
        assert_eq!(text, JSON);
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
    }

    #[test]
    fn utf16_with_bom_is_decoded() {
        assert_eq!(decode_text(&utf16_bytes(true)), JSON);
        assert_eq!(decode_text(&utf16_bytes(false)), JSON);
    }

    #[test]
    fn chinese_survives_both_encodings() {
        let text = "{\"name\":\"逻各斯\",\"id\":\"char_4132_ascln\"}";
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(text.as_bytes());
        assert_eq!(decode_text(&bom), text);

        let mut u16le = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            u16le.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_text(&u16le), text);
    }

    #[test]
    fn mid_file_bom_bytes_are_left_alone() {
        // 只有文件**开头**那三个字节算 BOM。正文里出现的同样字节是数据：
        // 它是合法 UTF-8（U+FEFF），解码后原样留在字符串里，不被当成 BOM 吃掉。
        let text = decode_text(&[b'{', 0xEF, 0xBB, 0xBF, b'}']);
        assert_eq!(text, "{\u{FEFF}}");
        // 但它不是 JSON 空白字符 → 该解析失败还是失败，不能悄悄"猜"用户想干嘛
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_err());
    }

    #[test]
    fn broken_utf8_degrades_instead_of_failing() {
        // 坏字节降级成替换字符，不再整份文件读不出来；JSON 会因此解析失败并给出正常报错
        let bytes = [b'{', 0xFF, 0xFE, 0x01, b'}']; // 注意：开头不是 BOM，因为第一个字节是 '{'
        let text = decode_text(&bytes);
        assert!(text.starts_with('{'));
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_err());
    }
}
