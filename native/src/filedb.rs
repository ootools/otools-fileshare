//! 共享条目按名称索引，原子更新并广播列表变更。

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::{event_sink, store};

const FILE_DB_KEY: &str = "FileDb";

fn normalize_db(value: &mut Value) -> Result<&mut Map<String, Value>, String> {
    if let Value::String(encoded) = value {
        *value = serde_json::from_str(encoded).map_err(|error| format!("共享列表损坏: {error}"))?;
    }
    value
        .as_object_mut()
        .ok_or_else(|| "共享列表格式错误".to_string())
}

fn changed() {
    event_sink::push("fileshare:fileDb.listChange", Value::Null);
    crate::server::sse::list_changed();
}

pub fn add_file(name: &str, path: &str, username: Option<&str>) -> Result<Value, String> {
    let path = Path::new(path)
        .canonicalize()
        .map_err(|error| format!("无法共享文件: {error}"))?;
    let metadata = path.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err("只能共享普通文件或文件夹".to_string());
    }
    let base = if metadata.is_dir() || name.is_empty() {
        path.file_name()
            .and_then(|part| part.to_str())
            .unwrap_or_default()
    } else {
        name
    };
    if base.is_empty() || base == "." || base == ".." || base.contains(['/', '\\', '\0']) {
        return Err("文件名称无效".to_string());
    }
    store::mutate_item(FILE_DB_KEY, json!({}), |db| {
        let db = normalize_db(db)?;
        let mut key = base.to_string();
        if metadata.is_dir() {
            let mut suffix = 1;
            while db
                .get(&key)
                .is_some_and(|entry| entry["path"] != json!(path))
            {
                key = format!("{base}_{suffix}");
                suffix += 1;
            }
        }
        let mut entry = json!({ "type": if metadata.is_dir() { "directory" } else { "file" }, "name":key, "path":path });
        if let Some(username) = username {
            entry["username"] = json!(username);
        }
        db.insert(key, entry);
        Ok(())
    })?;
    changed();
    Ok(json!({"success":true}))
}

pub fn add_text(text: &str, username: Option<&str>) -> Result<Value, String> {
    if text.is_empty() {
        return Err("消息不能为空".to_string());
    }
    let mut name: String = text.chars().take(20).collect();
    if text.chars().count() > 20 {
        name.push_str("...");
    }
    let intro: String = text.chars().take(100).collect();
    let mut entry = json!({"type":"text", "name":name, "content":text, "intro":intro});
    if let Some(username) = username {
        entry["username"] = json!(username);
    }
    store::mutate_item(FILE_DB_KEY, json!({}), |db| {
        normalize_db(db)?.insert(name, entry);
        Ok(())
    })?;
    changed();
    Ok(json!({"success":true}))
}

pub fn remove_file(name: &str) -> Result<(), String> {
    store::mutate_item(FILE_DB_KEY, json!({}), |db| {
        normalize_db(db)?.shift_remove(name);
        Ok(())
    })?;
    changed();
    Ok(())
}

pub fn list_files() -> Result<Vec<Value>, String> {
    let mut db = store::get_item_with_default(FILE_DB_KEY, json!({}))?;
    Ok(normalize_db(&mut db)?.values().cloned().collect())
}

pub fn get_file(name: &str) -> Result<Option<Value>, String> {
    let mut db = store::get_item_with_default(FILE_DB_KEY, json!({}))?;
    Ok(normalize_db(&mut db)?.get(name).cloned())
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use serde_json::json;

    #[test]
    fn shared_directories_deduplicate_names_and_reuse_the_same_path() {
        with_test_home(|home| {
            let first = home.join("a/photos");
            let second = home.join("b/photos");
            std::fs::create_dir_all(&first).unwrap();
            std::fs::create_dir_all(&second).unwrap();
            super::add_file("photos", first.to_str().unwrap(), None).unwrap();
            super::add_file("photos", second.to_str().unwrap(), None).unwrap();
            super::add_file("photos", second.to_str().unwrap(), None).unwrap();
            let files = super::list_files().unwrap();
            assert_eq!(files.len(), 2);
            assert_eq!(files[0]["name"], json!("photos"));
            assert_eq!(files[1]["name"], json!("photos_1"));
            assert_eq!(
                super::get_file("photos_1").unwrap().unwrap()["path"],
                json!(second.canonicalize().unwrap())
            );
        });
    }

    #[test]
    fn text_preview_preserves_unicode_and_removal_emits_list_change() {
        with_test_home(|_| {
            crate::event_sink::drain();
            let text = "你好世界🙂".repeat(25);
            super::add_text(&text, Some("local")).unwrap();
            let entries = super::list_files().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(
                entries[0]["name"],
                json!("你好世界🙂你好世界🙂你好世界🙂你好世界🙂...")
            );
            assert_eq!(entries[0]["intro"].as_str().unwrap().chars().count(), 100);
            assert_eq!(entries[0]["content"], json!(text));
            assert_eq!(entries[0]["username"], json!("local"));
            super::remove_file(entries[0]["name"].as_str().unwrap()).unwrap();
            assert!(super::list_files().unwrap().is_empty());
            let events = crate::event_sink::drain();
            assert_eq!(events.len(), 2);
            assert!(events
                .iter()
                .all(|event| event["topic"] == "fileshare:fileDb.listChange"));
        });
    }

    #[test]
    fn concurrent_file_shares_do_not_lose_entries() {
        with_test_home(|home| {
            std::thread::scope(|scope| {
                for index in 0..12 {
                    let path = home.join(format!("file-{index}.txt"));
                    std::fs::write(&path, "data").unwrap();
                    scope.spawn(move || {
                        super::add_file(
                            path.file_name().unwrap().to_str().unwrap(),
                            path.to_str().unwrap(),
                            None,
                        )
                        .unwrap();
                    });
                }
            });
            assert_eq!(super::list_files().unwrap().len(), 12);
        });
    }

    #[test]
    fn invalid_file_path_cannot_mutate_the_share_list() {
        with_test_home(|home| {
            assert!(
                super::add_file("missing", home.join("missing").to_str().unwrap(), None).is_err()
            );
            assert!(super::list_files().unwrap().is_empty());
        });
    }
}
