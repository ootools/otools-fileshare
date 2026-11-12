//! 路径只允许落在明确共享的文件或目录内。

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

use crate::filedb;

pub fn is_internal_name(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with(".otools-fileshare")
}

pub fn parse_path(filename: &str) -> Result<(PathBuf, Vec<String>), String> {
    let segments: Vec<String> = filename
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    if segments.is_empty() {
        return Err("文件路径为空".to_string());
    }
    if segments.iter().any(|part| {
        part == "." || part == ".." || is_internal_name(part) || part.contains(['\\', '\0', ':'])
    }) {
        return Err("禁止访问共享范围之外的路径".to_string());
    }
    let entry = filedb::get_file(&segments[0])?.ok_or("分享列表未找到该文件")?;
    let stored_root = PathBuf::from(entry["path"].as_str().ok_or("此条目不是文件")?);
    let root = stored_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    // 共享登记时已保存 canonical path；后续任何组件改成 symlink 不得重新定义边界。
    if root != stored_root
        || std::fs::symlink_metadata(&stored_root)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("共享根目录已改变".to_string());
    }
    let mut candidate = root.clone();
    if !root.is_dir() && segments.len() > 1 {
        return Err("不是文件夹".to_string());
    }
    for segment in &segments[1..] {
        if Path::new(segment)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err("文件路径无效".to_string());
        }
        candidate.push(segment);
    }
    let candidate = candidate
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !candidate.starts_with(&root)
        || candidate
            .components()
            .any(|component| is_internal_name(&component.as_os_str().to_string_lossy()))
    {
        return Err("禁止访问共享范围之外的路径".to_string());
    }
    Ok((candidate, segments))
}

pub fn list_dir(path: &Path) -> Result<Vec<Value>, String> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if is_internal_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_file() && !metadata.is_dir() {
            continue;
        }
        items.push(json!({"type":if metadata.is_dir(){"directory"}else{"file"}, "name":entry.file_name().to_string_lossy(), "path":entry.path(), "size":metadata.len()}));
    }
    Ok(items)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("otools-fileshare")
}
