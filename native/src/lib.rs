//! OTools fileshare 插件 —— native 侧入口。
//!
//! 面板经 `window.otools.invokeNative(method, payload)` 调进来，事件经
//! `poll_events` 轮询（宿主转发给前端 `listenNative`）。局域网 HTTP 服务器
//! 在后续任务挂到 [`runtime`] 与 [`dispatch`] 上。

pub mod config;
pub mod event_sink;
pub mod iputil;
pub mod runtime;
pub mod setting;
pub mod store;

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

/// 「宿主没下发插件目录」这条告警只记一次。
static PLUGIN_DIR_WARNED: AtomicBool = AtomicBool::new(false);

/// 结果包装约定（面板 shim 契约）：业务成功 `Ok(data)` →
/// `{"ok":true,"data":...}`；校验/存储失败 `Err(message)` →
/// `{"ok":false,"error":{"success":false,"message":...}}`（错误必须是对象
/// 而非字符串，shim 据此还原 Promise resolve / reject）。
fn ok_data(data: Result<Value, String>) -> Value {
    match data {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(message) => json!({
            "ok": false,
            "error": { "success": false, "message": message }
        }),
    }
}

/// 方法分发。面板 `invokeNative("方法名", payload)` 会落到这里；后续所有
/// 任务的方法都挂在这个 `match` 上。
pub fn dispatch(method: &str, payload: Value, plugin_dir: &str) -> Value {
    // plugin_dir 已由 invoke_impl 存入 config；plugin_dir 参数留给后续
    // 服务器任务消费（Task 8 起用于定位随包静态资源）。
    let _ = plugin_dir;

    match method {
        // 连通性探针：宿主 dev 冒烟 / 面板启动自检都用它。
        "ping" => json!({ "ok": true, "data": "pong" }),
        // 宿主事件泵：取出并清空已排队事件（{topic, payload} 数组）。
        "poll_events" => json!({ "ok": true, "data": { "events": event_sink::drain() } }),
        // ── Setting（Task 3）────────────────────────────────────────────
        // updateSetting / updateUploadPath / updateIp 的 payload 就是值本身
        // （对齐上游 api 签名：updateSetting(settings) / updateUploadPath(path)）。
        "getSetting" => ok_data(setting::get_setting()),
        // 逐键走对应 updater，Promise.all 语义（见 setting::update_setting）。
        // TODO(Task 8): 对齐上游 preload wrapper —— 端口变更时 stop+start
        // 服务并 emit fileshare:server.statusChange，当前不重启。
        "updateSetting" => {
            ok_data(setting::update_outcome_to_data(setting::update_setting(&payload)))
        }
        "getUrl" => ok_data(setting::get_url().map(Value::String)),
        "updateUploadPath" => {
            ok_data(setting::update_outcome_to_data(setting::update_upload_path(&payload)))
        }
        "updateIp" => ok_data(setting::update_outcome_to_data(setting::update_ip(&payload))),
        // 平台标识 / 版本号（面板「关于」页与语言探测用）。
        "getPlatform" => json!({ "ok": true, "data": "otools" }),
        "getVersion" => json!({ "ok": true, "data": env!("CARGO_PKG_VERSION") }),
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

    #[test]
    fn dispatch_setting_methods() {
        crate::store::test_support::with_test_home(|home| {
            std::fs::create_dir_all(home.join("up")).unwrap();
            assert_eq!(
                dispatch("getPlatform", Value::Null, ""),
                json!({ "ok": true, "data": "otools" })
            );
            assert_eq!(
                dispatch("getVersion", Value::Null, ""),
                json!({ "ok": true, "data": env!("CARGO_PKG_VERSION") })
            );
            assert_eq!(
                dispatch("getUrl", Value::Null, ""),
                json!({ "ok": true, "data": "http://127.0.0.1:5421" })
            );
            let setting = dispatch("getSetting", Value::Null, "");
            assert_eq!(setting["ok"], json!(true));
            assert_eq!(setting["data"]["port"], json!(5421));
            assert_eq!(setting["data"]["url"], json!("http://127.0.0.1:5421"));
            // 校验失败：reject 是对象错误 {success:false,message}，不是字符串
            let rejected = dispatch("updateUploadPath", json!(""), "");
            assert_eq!(
                rejected,
                json!({
                    "ok": false,
                    "error": { "success": false, "message": "更新上传路径失败，路径为空" }
                })
            );
            // 全键 updateSetting 成功 → 端口生效、url 跟随
            let payload = json!({
                "uploadPath": home.join("up").to_string_lossy(),
                "port": 6100,
                "authEnable": false,
                "tusEnable": false,
                "chunkSize": 20,
                "autoStart": false,
                "language": "zh",
            });
            assert_eq!(
                dispatch("updateSetting", payload, ""),
                json!({ "ok": true, "data": { "success": true, "message": "修改成功" } })
            );
            let setting = dispatch("getSetting", Value::Null, "");
            assert_eq!(setting["data"]["port"], json!(6100));
            assert_eq!(setting["data"]["url"], json!("http://127.0.0.1:6100"));
            // 运行时 IP 覆盖走 updateIp（不持久化）
            assert_eq!(
                dispatch("updateIp", json!("192.168.1.9"), ""),
                json!({ "ok": true, "data": { "success": true, "message": "修改成功" } })
            );
            assert_eq!(
                dispatch("getUrl", Value::Null, ""),
                json!({ "ok": true, "data": "http://192.168.1.9:6100" })
            );
        });
    }
}
