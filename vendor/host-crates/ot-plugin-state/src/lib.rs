use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

const OTOOLS_SYNC_DATA_DIR: &str = "data";
const DEFAULT_PLUGIN_STATE_FILE: &str = "state.json";

fn normalize_plugin_name(plugin: &str) -> String {
    let normalized = plugin
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

fn normalize_state_scheme(scheme: &str) -> Option<String> {
    let trimmed = scheme.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stem = trimmed.strip_suffix(".json").unwrap_or(trimmed);
    let normalized = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn plugin_state_file_name(scheme: Option<&str>) -> String {
    match scheme.and_then(normalize_state_scheme) {
        Some(name) => format!("{}.json", name),
        None => DEFAULT_PLUGIN_STATE_FILE.to_string(),
    }
}

fn normalize_state_key(key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn should_log_dbm_plugin_state(plugin: &str) -> bool {
    normalize_plugin_name(plugin) == "dbm"
}

fn format_state_scheme_label(scheme: Option<&str>) -> String {
    normalize_state_scheme(scheme.unwrap_or_default()).unwrap_or_else(|| "default".to_string())
}

fn summarize_state_value(value: &Value) -> String {
    match value {
        Value::Object(map) => format!("object_keys={}", map.len()),
        Value::Array(items) => format!("array_len={}", items.len()),
        Value::String(text) => format!("string_len={}", text.len()),
        Value::Number(_) => "number".to_string(),
        Value::Bool(flag) => format!("bool={}", flag),
        Value::Null => "null".to_string(),
    }
}

fn sync_state_root_dir() -> PathBuf {
    ot_storage::otools_root_dir().join(OTOOLS_SYNC_DATA_DIR)
}

pub fn plugin_state_path(plugin: &str) -> PathBuf {
    plugin_state_path_with_scheme(plugin, None)
}

pub fn plugin_state_path_with_scheme(plugin: &str, scheme: Option<&str>) -> PathBuf {
    ot_storage::otools_local_dir()
        .join(normalize_plugin_name(plugin))
        .join(plugin_state_file_name(scheme))
}

pub fn plugin_syncstate_path(plugin: &str) -> PathBuf {
    plugin_syncstate_path_with_scheme(plugin, None)
}

pub fn plugin_syncstate_path_with_scheme(plugin: &str, scheme: Option<&str>) -> PathBuf {
    sync_state_root_dir()
        .join(normalize_plugin_name(plugin))
        .join(plugin_state_file_name(scheme))
}

fn backup_corrupted_state_file(path: &Path, content: &str) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(DEFAULT_PLUGIN_STATE_FILE);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let backup_path = path.with_file_name(format!("{}.corrupt-{}.bak", file_name, timestamp));
    fs::write(&backup_path, content)
        .map_err(|error| format!("写入损坏状态备份失败({}): {}", backup_path.display(), error))?;
    Ok(backup_path)
}

fn read_state_json_with_repair<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("读取文件失败({}): {}", path.display(), error))?;

    match serde_json::from_str::<T>(&content) {
        Ok(value) => Ok(value),
        Err(original_error) => {
            let mut stream = serde_json::Deserializer::from_str(&content).into_iter::<Value>();
            let value = stream
                .next()
                .transpose()
                .map_err(|_| format!("解析 JSON 失败: {}", original_error))?
                .ok_or_else(|| format!("解析 JSON 失败: {}", original_error))?;
            let trailing = content[stream.byte_offset()..].trim();
            if trailing.is_empty() {
                return serde_json::from_value(value)
                    .map_err(|error| format!("解析 JSON 失败: {}", error));
            }

            let backup_path = backup_corrupted_state_file(path, &content)?;
            ot_storage::write_json_file(path, &value)?;
            eprintln!(
                "[plugin-state] repaired malformed json file={} backup={}",
                path.display(),
                backup_path.display()
            );

            serde_json::from_value(value).map_err(|error| format!("解析 JSON 失败: {}", error))
        }
    }
}

pub fn read_plugin_state<T: DeserializeOwned>(plugin: &str) -> Result<Option<T>, String> {
    read_plugin_state_with_scheme(plugin, None)
}

pub fn read_plugin_state_with_scheme<T: DeserializeOwned>(
    plugin: &str,
    scheme: Option<&str>,
) -> Result<Option<T>, String> {
    let state_path = plugin_state_path_with_scheme(plugin, scheme);
    if !state_path.exists() {
        return Ok(None);
    }
    read_state_json_with_repair(&state_path).map(Some)
}

pub fn save_plugin_localstate<T: Serialize>(plugin: &str, state: &T) -> Result<(), String> {
    save_plugin_localstate_with_scheme(plugin, None, state)
}

pub fn save_plugin_localstate_with_scheme<T: Serialize>(
    plugin: &str,
    scheme: Option<&str>,
    state: &T,
) -> Result<(), String> {
    let state_path = plugin_state_path_with_scheme(plugin, scheme);
    ot_storage::write_json_file(&state_path, state)
}

pub fn save_plugin_syncstate<T: Serialize>(plugin: &str, state: &T) -> Result<(), String> {
    save_plugin_syncstate_with_scheme(plugin, None, state)
}

pub fn save_plugin_syncstate_with_scheme<T: Serialize>(
    plugin: &str,
    scheme: Option<&str>,
    state: &T,
) -> Result<(), String> {
    let path = plugin_syncstate_path_with_scheme(plugin, scheme);
    ot_storage::write_json_file(&path, state)
}

pub fn read_plugin_syncstate<T: DeserializeOwned>(plugin: &str) -> Result<Option<T>, String> {
    read_plugin_syncstate_with_scheme(plugin, None)
}

pub fn read_plugin_syncstate_with_scheme<T: DeserializeOwned>(
    plugin: &str,
    scheme: Option<&str>,
) -> Result<Option<T>, String> {
    let state_path = plugin_syncstate_path_with_scheme(plugin, scheme);
    if !state_path.exists() {
        return Ok(None);
    }
    read_state_json_with_repair(&state_path).map(Some)
}

fn load_plugin_state_map_with_scheme(
    plugin: &str,
    scheme: Option<&str>,
) -> Result<Map<String, Value>, String> {
    let state = read_plugin_state_with_scheme::<Value>(plugin, scheme)?;
    match state {
        Some(Value::Object(map)) => Ok(map),
        Some(_) => Ok(Map::new()),
        None => Ok(Map::new()),
    }
}

fn load_plugin_syncstate_map_with_scheme(
    plugin: &str,
    scheme: Option<&str>,
) -> Result<Map<String, Value>, String> {
    let state = read_plugin_syncstate_with_scheme::<Value>(plugin, scheme)?;
    match state {
        Some(Value::Object(map)) => Ok(map),
        Some(_) => Ok(Map::new()),
        None => Ok(Map::new()),
    }
}

fn save_plugin_state_map_with_scheme(
    plugin: &str,
    scheme: Option<&str>,
    map: Map<String, Value>,
) -> Result<(), String> {
    save_plugin_localstate_with_scheme(plugin, scheme, &Value::Object(map))
}

fn save_plugin_syncstate_map_with_scheme(
    plugin: &str,
    scheme: Option<&str>,
    map: Map<String, Value>,
) -> Result<(), String> {
    save_plugin_syncstate_with_scheme(plugin, scheme, &Value::Object(map))
}

pub fn read_plugin_state_value<T: DeserializeOwned>(
    plugin: &str,
    key: &str,
) -> Result<Option<T>, String> {
    read_plugin_state_value_with_scheme(plugin, None, key)
}

pub fn read_plugin_syncstate_value<T: DeserializeOwned>(
    plugin: &str,
    key: &str,
) -> Result<Option<T>, String> {
    read_plugin_syncstate_value_with_scheme(plugin, None, key)
}

pub fn read_plugin_state_value_with_scheme<T: DeserializeOwned>(
    plugin: &str,
    scheme: Option<&str>,
    key: &str,
) -> Result<Option<T>, String> {
    let state_key = normalize_state_key(key).ok_or_else(|| "插件状态 key 不能为空".to_string())?;
    let state = load_plugin_state_map_with_scheme(plugin, scheme)?;
    let Some(value) = state.get(&state_key) else {
        return Ok(None);
    };

    serde_json::from_value::<T>(value.clone())
        .map(Some)
        .map_err(|error| format!("解析插件状态失败({}.{}): {}", plugin, state_key, error))
}

pub fn read_plugin_syncstate_value_with_scheme<T: DeserializeOwned>(
    plugin: &str,
    scheme: Option<&str>,
    key: &str,
) -> Result<Option<T>, String> {
    let state_key = normalize_state_key(key).ok_or_else(|| "插件状态 key 不能为空".to_string())?;
    let state = load_plugin_syncstate_map_with_scheme(plugin, scheme)?;
    let Some(value) = state.get(&state_key) else {
        return Ok(None);
    };

    serde_json::from_value::<T>(value.clone())
        .map(Some)
        .map_err(|error| format!("解析插件状态失败({}.{}): {}", plugin, state_key, error))
}

pub fn save_plugin_state_value<T: Serialize>(
    plugin: &str,
    key: &str,
    value: &T,
) -> Result<(), String> {
    save_plugin_state_value_with_scheme(plugin, None, key, value)
}

pub fn save_plugin_syncstate_value<T: Serialize>(
    plugin: &str,
    key: &str,
    value: &T,
) -> Result<(), String> {
    save_plugin_syncstate_value_with_scheme(plugin, None, key, value)
}

pub fn save_plugin_state_value_with_scheme<T: Serialize>(
    plugin: &str,
    scheme: Option<&str>,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let state_key = normalize_state_key(key).ok_or_else(|| "插件状态 key 不能为空".to_string())?;
    let mut state = load_plugin_state_map_with_scheme(plugin, scheme)?;
    state.insert(
        state_key,
        serde_json::to_value(value).map_err(|error| format!("序列化插件状态失败: {}", error))?,
    );
    save_plugin_state_map_with_scheme(plugin, scheme, state)
}

pub fn save_plugin_syncstate_value_with_scheme<T: Serialize>(
    plugin: &str,
    scheme: Option<&str>,
    key: &str,
    value: &T,
) -> Result<(), String> {
    let state_key = normalize_state_key(key).ok_or_else(|| "插件状态 key 不能为空".to_string())?;
    let mut state = load_plugin_syncstate_map_with_scheme(plugin, scheme)?;
    state.insert(
        state_key,
        serde_json::to_value(value).map_err(|error| format!("序列化插件状态失败: {}", error))?,
    );
    save_plugin_syncstate_map_with_scheme(plugin, scheme, state)
}

pub fn merge_plugin_state(plugin: &str, patch: Map<String, Value>) -> Result<(), String> {
    merge_plugin_state_with_scheme(plugin, None, patch)
}

pub fn merge_plugin_syncstate(plugin: &str, patch: Map<String, Value>) -> Result<(), String> {
    merge_plugin_syncstate_with_scheme(plugin, None, patch)
}

pub fn merge_plugin_state_with_scheme(
    plugin: &str,
    scheme: Option<&str>,
    patch: Map<String, Value>,
) -> Result<(), String> {
    let mut state = load_plugin_state_map_with_scheme(plugin, scheme)?;
    for (key, value) in patch {
        if normalize_state_key(&key).is_some() {
            state.insert(key, value);
        }
    }
    save_plugin_state_map_with_scheme(plugin, scheme, state)
}

pub fn merge_plugin_syncstate_with_scheme(
    plugin: &str,
    scheme: Option<&str>,
    patch: Map<String, Value>,
) -> Result<(), String> {
    let mut state = load_plugin_syncstate_map_with_scheme(plugin, scheme)?;
    for (key, value) in patch {
        if normalize_state_key(&key).is_some() {
            state.insert(key, value);
        }
    }
    save_plugin_syncstate_map_with_scheme(plugin, scheme, state)
}

pub fn get_otools_plugin_localstate(plugin: String) -> Result<Option<Value>, String> {
    read_plugin_state::<Value>(&plugin)
}

pub fn save_otools_plugin_localstate(plugin: String, state: Value) -> Result<(), String> {
    save_plugin_localstate(&plugin, &state)
}

pub fn get_otools_plugin_localstate_with_scheme(
    plugin: String,
    scheme: Option<String>,
) -> Result<Option<Value>, String> {
    let should_log = should_log_dbm_plugin_state(&plugin);
    let started_at = Instant::now();

    if should_log {
        println!(
            "[dbm][state][local] read:start plugin={} scheme={}",
            plugin,
            format_state_scheme_label(scheme.as_deref())
        );
    }

    let result = read_plugin_state_with_scheme::<Value>(&plugin, scheme.as_deref());

    if should_log {
        match &result {
            Ok(Some(value)) => {
                println!(
                    "[dbm][state][local] read:done plugin={} scheme={} summary={} elapsed_ms={}",
                    plugin,
                    format_state_scheme_label(scheme.as_deref()),
                    summarize_state_value(value),
                    started_at.elapsed().as_millis()
                );
            }
            Ok(None) => {
                println!(
                    "[dbm][state][local] read:empty plugin={} scheme={} elapsed_ms={}",
                    plugin,
                    format_state_scheme_label(scheme.as_deref()),
                    started_at.elapsed().as_millis()
                );
            }
            Err(error) => {
                eprintln!(
                    "[dbm][state][local] read:failed plugin={} scheme={} error={} elapsed_ms={}",
                    plugin,
                    format_state_scheme_label(scheme.as_deref()),
                    error,
                    started_at.elapsed().as_millis()
                );
            }
        }
    }

    result
}

pub fn save_otools_plugin_localstate_with_scheme(
    plugin: String,
    scheme: Option<String>,
    state: Value,
) -> Result<(), String> {
    let should_log = should_log_dbm_plugin_state(&plugin);
    let started_at = Instant::now();

    if should_log {
        println!(
            "[dbm][state][local] write:start plugin={} scheme={} summary={}",
            plugin,
            format_state_scheme_label(scheme.as_deref()),
            summarize_state_value(&state)
        );
    }

    let result = save_plugin_localstate_with_scheme(&plugin, scheme.as_deref(), &state);

    if should_log {
        match &result {
            Ok(()) => {
                println!(
                    "[dbm][state][local] write:done plugin={} scheme={} elapsed_ms={}",
                    plugin,
                    format_state_scheme_label(scheme.as_deref()),
                    started_at.elapsed().as_millis()
                );
            }
            Err(error) => {
                eprintln!(
                    "[dbm][state][local] write:failed plugin={} scheme={} error={} elapsed_ms={}",
                    plugin,
                    format_state_scheme_label(scheme.as_deref()),
                    error,
                    started_at.elapsed().as_millis()
                );
            }
        }
    }

    result
}

pub fn get_otools_plugin_syncstate(plugin: String) -> Result<Option<Value>, String> {
    read_plugin_syncstate::<Value>(&plugin)
}

pub fn save_otools_plugin_syncstate(plugin: String, state: Value) -> Result<(), String> {
    save_plugin_syncstate(&plugin, &state)
}

pub fn get_otools_plugin_syncstate_with_scheme(
    plugin: String,
    scheme: Option<String>,
) -> Result<Option<Value>, String> {
    read_plugin_syncstate_with_scheme::<Value>(&plugin, scheme.as_deref())
}

pub fn save_otools_plugin_syncstate_with_scheme(
    plugin: String,
    scheme: Option<String>,
    state: Value,
) -> Result<(), String> {
    save_plugin_syncstate_with_scheme(&plugin, scheme.as_deref(), &state)
}

pub fn get_otools_plugin_localstate_value(
    plugin: String,
    key: String,
) -> Result<Option<Value>, String> {
    read_plugin_state_value::<Value>(&plugin, &key)
}

pub fn save_otools_plugin_localstate_value(
    plugin: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    save_plugin_state_value(&plugin, &key, &value)
}

pub fn patch_otools_plugin_localstate(plugin: String, patch: Value) -> Result<(), String> {
    let Value::Object(patch_map) = patch else {
        return Err("patch 必须是对象".to_string());
    };
    merge_plugin_state(&plugin, patch_map)
}

pub fn get_otools_plugin_localstate_value_with_scheme(
    plugin: String,
    scheme: Option<String>,
    key: String,
) -> Result<Option<Value>, String> {
    read_plugin_state_value_with_scheme::<Value>(&plugin, scheme.as_deref(), &key)
}

pub fn save_otools_plugin_localstate_value_with_scheme(
    plugin: String,
    scheme: Option<String>,
    key: String,
    value: Value,
) -> Result<(), String> {
    save_plugin_state_value_with_scheme(&plugin, scheme.as_deref(), &key, &value)
}

pub fn patch_otools_plugin_localstate_with_scheme(
    plugin: String,
    scheme: Option<String>,
    patch: Value,
) -> Result<(), String> {
    let Value::Object(patch_map) = patch else {
        return Err("patch 必须是对象".to_string());
    };
    merge_plugin_state_with_scheme(&plugin, scheme.as_deref(), patch_map)
}

pub fn get_otools_plugin_syncstate_value(
    plugin: String,
    key: String,
) -> Result<Option<Value>, String> {
    read_plugin_syncstate_value::<Value>(&plugin, &key)
}

pub fn save_otools_plugin_syncstate_value(
    plugin: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    save_plugin_syncstate_value(&plugin, &key, &value)
}

pub fn patch_otools_plugin_syncstate(plugin: String, patch: Value) -> Result<(), String> {
    let Value::Object(patch_map) = patch else {
        return Err("patch 必须是对象".to_string());
    };
    merge_plugin_syncstate(&plugin, patch_map)
}

pub fn get_otools_plugin_syncstate_value_with_scheme(
    plugin: String,
    scheme: Option<String>,
    key: String,
) -> Result<Option<Value>, String> {
    read_plugin_syncstate_value_with_scheme::<Value>(&plugin, scheme.as_deref(), &key)
}

pub fn save_otools_plugin_syncstate_value_with_scheme(
    plugin: String,
    scheme: Option<String>,
    key: String,
    value: Value,
) -> Result<(), String> {
    save_plugin_syncstate_value_with_scheme(&plugin, scheme.as_deref(), &key, &value)
}

pub fn patch_otools_plugin_syncstate_with_scheme(
    plugin: String,
    scheme: Option<String>,
    patch: Value,
) -> Result<(), String> {
    let Value::Object(patch_map) = patch else {
        return Err("patch 必须是对象".to_string());
    };
    merge_plugin_syncstate_with_scheme(&plugin, scheme.as_deref(), patch_map)
}
