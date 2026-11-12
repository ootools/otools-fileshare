//! 插件 K-V 存储层。
//!
//! 落盘走宿主的 `ot-plugin-state`（state.json 单文件，路径
//! `~/.otools/local/otools-fileshare/state.json`，根目录由 ot-storage 按
//! `HOME` 环境变量决定）。`ot-plugin-state` 每次写入都是「读全量 map →
//! 改 → 写全量」，并发写会互相覆盖丢键，因此这里用进程级 `Mutex` 把全部
//! 读写串行化。

use ot_plugin_state::{
    read_plugin_state, read_plugin_state_value, save_plugin_localstate, save_plugin_state_value,
};
use serde_json::Value;
use std::sync::{Mutex, MutexGuard};

/// 插件固定 id：ot-plugin-state 以它决定落盘子目录（小写归一化后同名）。
const PLUGIN_ID: &str = "otools-fileshare";

/// 进程内读写串行锁。poison 时继续用剩余数据，别让一次 panic 毒死存储。
static STORE_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    STORE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 读取一个 K-V 项；键不存在返回 `None`，读失败（IO/解析）返回 `Err`。
pub fn get_item(key: &str) -> Result<Option<Value>, String> {
    let _guard = lock();
    read_plugin_state_value(PLUGIN_ID, key)
}

/// 写入一个 K-V 项。
pub fn set_item(key: &str, value: Value) -> Result<(), String> {
    let _guard = lock();
    save_plugin_state_value(PLUGIN_ID, key, &value)
}

/// 删除一个 K-V 项。ot-plugin-state 只提供 insert 语义（save/merge 都无法
/// 去键，存 null 也不等于删除），所以删除 = 读全量 map → 去键 →
/// `save_plugin_localstate` 整体重写；同一把锁内完成，没有并发窗口。
pub fn remove_item(key: &str) -> Result<(), String> {
    let _guard = lock();
    match read_plugin_state::<Value>(PLUGIN_ID)? {
        Some(Value::Object(mut map)) => {
            if map.remove(key).is_some() {
                save_plugin_localstate(PLUGIN_ID, &Value::Object(map))
            } else {
                Ok(())
            }
        }
        // 整个 state 不存在或不是对象 → 没有可删的键，按已删除处理
        _ => Ok(()),
    }
}

/// 读取带默认值（对齐上游 PlatformAdapter 的 getStorageItem 语义）：
/// 键缺失或存了 null → 返回 default；default 为 null 时直接返回 null、
/// 不落盘；否则先持久化 default 再返回（上游首读即写默认值）。
pub fn get_item_with_default(key: &str, default: Value) -> Result<Value, String> {
    let _guard = lock();
    match read_plugin_state_value::<Value>(PLUGIN_ID, key)? {
        Some(value) if !value.is_null() => Ok(value),
        _ => {
            if default.is_null() {
                return Ok(Value::Null);
            }
            save_plugin_state_value(PLUGIN_ID, key, &default)?;
            Ok(default)
        }
    }
}

/// 原子修改一个条目，避免业务模块读改写时丢失并发更新。
pub fn mutate_item<T>(
    key: &str,
    default: Value,
    edit: impl FnOnce(&mut Value) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = lock();
    let mut value = read_plugin_state_value::<Value>(PLUGIN_ID, key)?.unwrap_or(default);
    let result = edit(&mut value)?;
    save_plugin_state_value(PLUGIN_ID, key, &value)?;
    Ok(result)
}

// ── 测试支持 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Mutex, MutexGuard};

    /// ot-storage 的落盘根由 `HOME` 决定（`~/.otools/...`），而环境变量是
    /// 进程全局的：并行测试同时改 `HOME` 会互相踩。用这把锁把所有触盘测试
    /// 串行化，并给每个测试一次性 tempdir（进程 id + 递增序号，跨进程也不重名）。
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    static DIR_SEQ: AtomicU32 = AtomicU32::new(0);

    pub(crate) fn with_test_home<T>(f: impl FnOnce(&Path) -> T) -> T {
        let guard: MutexGuard<'_, ()> = HOME_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let old_home = std::env::var_os("HOME");
        let old_otools_home = std::env::var_os("OTOOLS_HOME");
        let home: PathBuf = std::env::temp_dir().join(format!(
            "otools-fileshare-test-{}-{}",
            std::process::id(),
            DIR_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&home).expect("创建测试 HOME 失败");
        std::env::set_var("HOME", &home);
        // OTOOLS_HOME 是本插件 ot-storage 快照补丁的落盘根覆盖：
        // Windows 上 dirs::home_dir() 不读 HOME，只有它能在全平台隔离 state.json。
        std::env::set_var("OTOOLS_HOME", &home);
        // 运行时态（setting 的 curIp 覆盖）一并隔离，避免用例间串扰。
        crate::setting::test_reset_cur_ip();
        crate::test_reset_shutdown_gate();
        let result = f(&home);
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match old_otools_home {
            Some(value) => std::env::set_var("OTOOLS_HOME", value),
            None => std::env::remove_var("OTOOLS_HOME"),
        }
        drop(guard);
        let _ = std::fs::remove_dir_all(&home);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::with_test_home;
    use serde_json::{json, Value};

    #[test]
    fn set_get_remove_roundtrip() {
        with_test_home(|_| {
            assert_eq!(super::get_item("k1").unwrap(), None);
            super::set_item("k1", json!({ "a": 1 })).unwrap();
            assert_eq!(super::get_item("k1").unwrap(), Some(json!({ "a": 1 })));
            super::set_item("k2", json!("v2")).unwrap();
            super::remove_item("k1").unwrap();
            assert_eq!(super::get_item("k1").unwrap(), None);
            assert_eq!(super::get_item("k2").unwrap(), Some(json!("v2")));
        });
    }

    #[test]
    fn state_lands_on_disk_under_isolated_home() {
        with_test_home(|home| {
            super::set_item("port", json!(5421)).unwrap();
            let state_file = home.join(".otools/local/otools-fileshare/state.json");
            assert!(
                state_file.exists(),
                "state.json 应落在测试 HOME 下: {}",
                state_file.display()
            );
        });
    }

    #[test]
    fn remove_missing_key_is_noop() {
        with_test_home(|_| {
            assert!(super::remove_item("ghost").is_ok());
            assert_eq!(super::get_item("ghost").unwrap(), None);
        });
    }

    #[test]
    fn default_persists_on_first_read() {
        with_test_home(|_| {
            assert_eq!(
                super::get_item_with_default("p", json!(5421)).unwrap(),
                json!(5421)
            );
            assert_eq!(
                super::get_item("p").unwrap(),
                Some(json!(5421)),
                "上游 adapter 首读即持久化默认值"
            );
            // null 默认值不落盘（上游 getStorageItem 对 null 默认值直接返回 null）
            assert_eq!(
                super::get_item_with_default("lang", Value::Null).unwrap(),
                Value::Null
            );
            assert_eq!(super::get_item("lang").unwrap(), None);
        });
    }
}
