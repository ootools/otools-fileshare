use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const OTOOLS_DIR: &str = ".otools";
const OTOOLS_LOCAL_DATA_DIR: &str = "local";
const OTOOLS_LOGS_DIR: &str = "logs";

fn home_dir() -> PathBuf {
    // 本插件快照补丁：OTOOLS_HOME 供测试隔离落盘根。
    // Windows 上 dirs::home_dir() 走系统用户目录、不读 HOME，仅设 HOME 无法隔离。
    std::env::var_os("OTOOLS_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| dirs::home_dir())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn normalize_scope_name(raw: &str) -> String {
    let normalized = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {}", error))?;
    }
    Ok(())
}

pub fn otools_root_dir() -> PathBuf {
    home_dir().join(OTOOLS_DIR)
}

pub fn otools_local_dir() -> PathBuf {
    otools_root_dir().join(OTOOLS_LOCAL_DATA_DIR)
}

pub fn tool_local_dir(tool: &str) -> PathBuf {
    otools_local_dir().join(normalize_scope_name(tool))
}

pub fn otools_logs_dir() -> PathBuf {
    otools_root_dir().join(OTOOLS_LOGS_DIR)
}

pub fn tool_logs_dir(tool: &str) -> PathBuf {
    otools_logs_dir().join(normalize_scope_name(tool))
}

pub fn tool_log_path(tool: &str, file_name: &str) -> PathBuf {
    tool_logs_dir(tool).join(file_name)
}

pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = fs::read_to_string(path).map_err(|error| format!("读取文件失败: {}", error))?;
    serde_json::from_str(&content).map_err(|error| format!("解析 JSON 失败: {}", error))
}

pub fn read_text_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("读取文件失败({}): {}", path.display(), error))
}

pub fn write_json_file<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("序列化 JSON 失败: {}", error))?;
    fs::write(path, content).map_err(|error| format!("写入文件失败: {}", error))
}

pub fn write_text_file(path: &Path, content: &str) -> Result<(), String> {
    ensure_parent_dir(path)?;
    fs::write(path, content).map_err(|error| format!("写入文件失败({}): {}", path.display(), error))
}
