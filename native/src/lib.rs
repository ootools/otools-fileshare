//! OTools fileshare 插件 —— native 侧入口。
//!
//! 面板经 `window.otools.invokeNative(method, payload)` 调进来，事件经
//! `poll_events` 轮询（宿主转发给前端 `listenNative`）。局域网 HTTP 服务器
//! 在后续任务挂到 [`runtime`] 与 [`dispatch`] 上。

pub mod config;
pub mod event_sink;
pub mod runtime;

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

/// 「宿主没下发插件目录」这条告警只记一次。
static PLUGIN_DIR_WARNED: AtomicBool = AtomicBool::new(false);

/// 方法分发。面板 `invokeNative("方法名", payload)` 会落到这里；后续所有
/// 任务的方法都挂在这个 `match` 上。
pub fn dispatch(method: &str, payload: Value, plugin_dir: &str) -> Value {
    // plugin_dir 已由 invoke_impl 存入 config；payload / plugin_dir 两参留给
    // 后续任务的方法消费（本任务只有 ping / poll_events / shutdown 三个方法）。
    let _ = payload;
    let _ = plugin_dir;

    match method {
        // 连通性探针：宿主 dev 冒烟 / 面板启动自检都用它。
        "ping" => json!({ "ok": true, "data": "pong" }),
        // 宿主事件泵：取出并清空已排队事件（{topic, payload} 数组）。
        "poll_events" => json!({ "ok": true, "data": { "events": event_sink::drain() } }),
        // 宿主在升级 / 重装插件前会先来问一次能不能停：关掉 tokio runtime，
        // 让 worker 线程自行退出（后台关闭，不阻塞宿主调用线程）。
        "__otools_shutdown__" => {
            runtime::shutdown();
            json!({ "ok": true, "data": { "ok": true } })
        }
        other => json!({ "ok": false, "error": format!("unknown method: {other}") }),
    }
}

// ── native 插件 ABI ─────────────────────────────────────────────────────────

fn invoke_impl(input_ptr: *const u8, input_len: usize, output_len: *mut usize) -> *mut u8 {
    if input_ptr.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }
    let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };

    // 非法 JSON 直接回错误包（区别于「非对象输入按空 envelope 处理」）。
    let parsed: Value = match serde_json::from_slice(input) {
        Ok(value) => value,
        Err(error) => {
            let encoded = serde_json::to_vec(&json!({
                "ok": false,
                "error": format!("invalid input: {error}"),
            }))
            .unwrap_or_else(|_| br#"{"ok":false,"error":"serialize failed"}"#.to_vec());
            return ot_plugin_ffi::take_json_response(encoded, output_len);
        }
    };

    // 宿主 envelope：{"method": string, "payload": any|null, "pluginDir": string}。
    // 非对象输入按空 envelope 处理（等价 unknown method）。
    let (method, payload, plugin_dir) = match parsed {
        Value::Object(ref map) => (
            map.get("method")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            map.get("payload").cloned().unwrap_or(Value::Null),
            map.get("pluginDir")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
        _ => (String::new(), Value::Null, String::new()),
    };

    // 宿主每次调用都会带上插件安装目录（后续任务用它定位随包静态资源）。
    // 空串时保留旧值；连旧值都没有则推一条 warn-once 日志事件。
    if !plugin_dir.is_empty() {
        config::set_plugin_dir(&plugin_dir);
    } else if config::plugin_dir().is_empty() && !PLUGIN_DIR_WARNED.swap(true, Ordering::Relaxed) {
        event_sink::push("fileshare:log", json!({ "warning": "pluginDir missing" }));
    }

    let response = dispatch(&method, payload, &plugin_dir);
    let encoded = serde_json::to_vec(&response)
        .unwrap_or_else(|_| br#"{"ok":false,"error":"serialize failed"}"#.to_vec());
    ot_plugin_ffi::take_json_response(encoded, output_len)
}

/// 宿主唯一调用入口。插件内部 panic 无法跨 DLL unwind（原本会走
/// panic_cannot_unwind → abort，宿主整个进程被杀），所以必须在边界内捕获。
#[no_mangle]
pub extern "C" fn otools_plugin_invoke(
    input_ptr: *const u8,
    input_len: usize,
    output_len: *mut usize,
) -> *mut u8 {
    ot_plugin_ffi::guard_owned_invoke(
        env!("CARGO_PKG_NAME"),
        input_ptr,
        input_len,
        output_len,
        invoke_impl,
    )
}

#[no_mangle]
pub extern "C" fn otools_plugin_free(ptr: *mut u8, len: usize) {
    // 释放同样放进 ffi crate：Vec::from_raw_parts 的长度不匹配是 UB，
    // 真出问题时报错比静默未定义行为好。
    ot_plugin_ffi::free_response(ptr, len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn dispatch_ping_ok() {
        assert_eq!(
            dispatch("ping", Value::Null, ""),
            json!({ "ok": true, "data": "pong" })
        );
    }

    #[test]
    fn dispatch_unknown_method() {
        assert_eq!(dispatch("nope", Value::Null, "")["ok"], json!(false));
    }

    #[test]
    fn poll_events_drains() {
        event_sink::push("fileshare:test", json!(1));
        let evs = dispatch("poll_events", Value::Null, "");
        assert_eq!(evs["data"]["events"][0]["topic"], json!("fileshare:test"));
        // drain 后为空
        let again = dispatch("poll_events", Value::Null, "");
        assert!(
            again["data"]["events"].as_array().unwrap().is_empty(),
            "second poll_events must not repeat drained events: {again}"
        );
    }
}
