//! invoke 环境信息（插件安装目录）的存放处。
//!
//! 宿主每次 `otools_plugin_invoke` 都会在 envelope 里带 `pluginDir`（插件
//! 安装目录）；后续任务用它定位随包资源（`<pluginDir>/web-static/` 静态页）。

use std::sync::RwLock;

static PLUGIN_DIR: RwLock<String> = RwLock::new(String::new());

/// 记录宿主下发的插件目录；空串忽略（保留旧值）。
pub fn set_plugin_dir(dir: &str) {
    if dir.trim().is_empty() {
        return;
    }
    if let Ok(mut guard) = PLUGIN_DIR.write() {
        *guard = dir.to_string();
    }
}

/// 当前插件目录；宿主尚未下发时为空串。
pub fn plugin_dir() -> String {
    PLUGIN_DIR
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}
