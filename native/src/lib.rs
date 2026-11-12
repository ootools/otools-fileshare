//! OTools fileshare 插件 —— native 侧入口。
//!
//! 面板经 `window.otools.invokeNative(method, payload)` 调进来，事件经
//! `poll_events` 轮询（宿主转发给前端 `listenNative`）。局域网 HTTP 服务器
//! 在后续任务挂到 [`runtime`] 与 [`dispatch`] 上。

pub mod account;
pub mod config;
pub mod event_sink;
pub mod filedb;
pub mod iputil;
pub mod jwt;
pub mod runtime;
pub mod server;
pub mod setting;
pub mod store;
pub mod ziputil;

#[cfg(test)]
mod security_tests;

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
static INVOKE_GATE: RwLock<bool> = RwLock::new(false);

/// 「宿主没下发插件目录」这条告警只记一次。
static PLUGIN_DIR_WARNED: AtomicBool = AtomicBool::new(false);

/// 结果包装约定（面板 shim 契约）：业务成功 `Ok(data)` →
/// `{"ok":true,"data":...}`；校验/存储失败 `Err(message)` →
/// `{"ok":false,"error":message}`；宿主 native IPC 只保留字符串错误。
/// 面板收到 rejection 后统一转换成 Error(message)。
fn ok_data(data: Result<Value, String>) -> Value {
    match data {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(message) => json!({ "ok": false, "error": message }),
    }
}

fn platform_open(target: &str, reveal: bool) -> Result<(), String> {
    use std::process::Command;
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        if reveal {
            command.arg("-R");
        }
        command.arg(target);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        if reveal {
            command.arg(format!("/select,{target}"));
        } else {
            command.arg(target);
        }
        command
    };
    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(if reveal {
            std::path::Path::new(target)
                .parent()
                .unwrap_or(std::path::Path::new(target))
                .to_string_lossy()
                .into_owned()
        } else {
            target.to_string()
        });
        command
    };
    let status = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("打开失败: {status}"))
    }
}

fn string_arg<'a>(payload: &'a Value, key: &str) -> &'a str {
    payload
        .get(key)
        .and_then(Value::as_str)
        .or_else(|| payload.as_str())
        .unwrap_or_default()
}

fn family_arg(payload: &Value) -> &str {
    payload
        .get("family")
        .or_else(|| payload.get("ipFamily"))
        .and_then(Value::as_str)
        .or_else(|| payload.as_str())
        .unwrap_or("ipv4")
}

/// 方法分发。面板 `invokeNative("方法名", payload)` 会落到这里；后续所有
/// 任务的方法都挂在这个 `match` 上。
pub fn dispatch(method: &str, payload: Value, plugin_dir: &str) -> Value {
    if method == "__otools_shutdown__" {
        let mut gate = INVOKE_GATE
            .write()
            .unwrap_or_else(|error| error.into_inner());
        *gate = true;
        if let Err(error) = server::stop() {
            return ok_data(Err(error));
        }
        runtime::shutdown();
        return json!({"ok":true,"data":{"ok":true}});
    }
    let gate = INVOKE_GATE
        .read()
        .unwrap_or_else(|error| error.into_inner());
    if *gate && method != "poll_events" {
        return ok_data(Err("插件已关闭，请重新加载".to_string()));
    }
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
            let old_port = setting::get_port();
            let was_running = server::status() == "start";
            let result = setting::update_setting(&payload).and_then(|message| {
                if let Some(allow) = payload
                    .get("allowAnonymousUpload")
                    .filter(|value| value.is_boolean())
                {
                    store::set_item("allowAnonymousUpload", allow.clone())?;
                }
                if was_running && old_port? != setting::get_port()? {
                    server::stop()?;
                    server::start()?;
                }
                Ok(message)
            });
            ok_data(setting::update_outcome_to_data(result))
        }
        "getUrl" => ok_data(setting::get_url().map(Value::String)),
        "getIp" => ok_data(iputil::get_ip().map(Value::String)),
        "getIpFamily" => ok_data(iputil::get_ip_family().map(Value::String)),
        "getNetInterface" => ok_data(iputil::get_net_interface().map(Value::String)),
        "getNetInterfaceNames" => {
            ok_data(iputil::get_net_interface_names(family_arg(&payload)).map(|names| json!(names)))
        }
        "getIpAddresses" => ok_data(
            iputil::get_ip_addresses(
                string_arg(&payload, "netInterfaceName"),
                family_arg(&payload),
            )
            .map(|items| json!(items)),
        ),
        "getIpAddress" => ok_data(
            iputil::get_ip_address(
                payload.get("idx").and_then(Value::as_u64).unwrap_or(0) as usize,
                family_arg(&payload),
            )
            .map(Value::String),
        ),
        "setIpFamily" => ok_data(iputil::set_ip_family(family_arg(&payload)).map(|_| Value::Null)),
        "setNetInterface" => ok_data(
            iputil::set_net_interface(string_arg(&payload, "netInterfaceName"))
                .map(|_| Value::Null),
        ),
        "updateUploadPath" => ok_data(setting::update_outcome_to_data(
            setting::update_upload_path(&payload),
        )),
        "updateIp" => ok_data(setting::update_outcome_to_data(setting::update_ip(
            &payload,
        ))),
        // 平台标识 / 版本号（面板「关于」页与语言探测用）。
        "openFile" => ok_data(filedb::get_file(string_arg(&payload, "filename")).and_then(
            |entry| {
                let entry = entry.ok_or("分享列表未找到该文件")?;
                let path = entry["path"].as_str().ok_or("此条目不是文件")?;
                platform_open(path, true).map(|_| Value::Null)
            },
        )),
        "openExternal" => {
            let url = string_arg(&payload, "url");
            if !url.starts_with("https://") && !url.starts_with("http://") {
                ok_data(Err("只支持 HTTP/HTTPS 地址".to_string()))
            } else {
                ok_data(platform_open(url, false).map(|_| Value::Null))
            }
        }
        "openDevTool" | "closeDevTool" => {
            ok_data(Ok(json!({"success":false,"message":"not supported"})))
        }
        "getToken" => {
            let permanent = payload
                .get("permanent")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let timeout = payload
                .get("timeoutSec")
                .and_then(Value::as_i64)
                .unwrap_or(3600);
            ok_data(Ok(Value::String(jwt::sign(
                json!({"permissions":jwt::admin_permissions()}),
                if permanent { 0 } else { timeout },
            ))))
        }
        "listAccounts" => ok_data(account::list_accounts().map(|accounts| json!(accounts))),
        "addAccount" => ok_data(account::add_account(&payload)),
        "updateAccount" => ok_data(account::update_account(
            string_arg(&payload, "id"),
            payload.get("account").unwrap_or(&payload),
        )),
        "updateAccountPassword" => ok_data(account::update_account_password(
            string_arg(&payload, "id"),
            string_arg(&payload, "password"),
        )),
        "removeAccount" => ok_data(account::remove_account(string_arg(&payload, "id"))),
        "listFiles" => ok_data(filedb::list_files().map(|files| json!(files))),
        "addFile" => ok_data(filedb::add_file(
            string_arg(&payload, "name"),
            string_arg(&payload, "path"),
            payload.get("username").and_then(Value::as_str),
        )),
        "addText" => ok_data(filedb::add_text(
            string_arg(&payload, "text"),
            payload.get("username").and_then(Value::as_str),
        )),
        "removeFile" => {
            ok_data(filedb::remove_file(string_arg(&payload, "name")).map(|_| Value::Null))
        }
        "getPlatform" => json!({ "ok": true, "data": "otools" }),
        "getVersion" => json!({ "ok": true, "data": env!("CARGO_PKG_VERSION") }),
        // 宿主在升级 / 重装插件前会先来问一次能不能停：关掉 tokio runtime，
        // 让 worker 线程自行退出（后台关闭，不阻塞宿主调用线程）。
        "startServer" => ok_data(server::start()),
        "stopServer" => ok_data(server::stop()),
        "getServerStatus" => ok_data(Ok(json!(server::status()))),
        "__otools_shutdown__" => {
            if let Err(error) = server::stop() {
                return ok_data(Err(error));
            }
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
pub(crate) fn test_reset_shutdown_gate() {
    *INVOKE_GATE
        .write()
        .unwrap_or_else(|error| error.into_inner()) = false;
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
        crate::store::test_support::with_test_home(|_| {
            event_sink::drain();
            event_sink::push("fileshare:test", json!(1));
            let evs = dispatch("poll_events", Value::Null, "");
            assert_eq!(evs["data"]["events"][0]["topic"], json!("fileshare:test"));
            // drain 后为空
            let again = dispatch("poll_events", Value::Null, "");
            assert!(
                again["data"]["events"].as_array().unwrap().is_empty(),
                "second poll_events must not repeat drained events: {again}"
            );
        });
    }

    #[test]
    fn full_method_surface_and_port_restart() {
        crate::store::test_support::with_test_home(|home| {
            for method in ["openFile", "openExternal", "openDevTool", "closeDevTool"] {
                let result = dispatch(method, Value::Null, "");
                assert!(
                    !result["error"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("unknown method"),
                    "missing method: {method}"
                );
            }
            let page = home.join("plugin/web-static/page_web");
            std::fs::create_dir_all(&page).unwrap();
            std::fs::write(page.join("index.html"), "<html>test</html>").unwrap();
            config::set_plugin_dir(home.join("plugin").to_str().unwrap());
            let upload = home.join("up");
            std::fs::create_dir_all(&upload).unwrap();
            let first = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let second = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port1 = first.local_addr().unwrap().port();
            let port2 = second.local_addr().unwrap().port();
            drop(first);
            drop(second);
            setting::update_port(&json!(port1)).unwrap();
            assert_eq!(dispatch("startServer", Value::Null, "")["ok"], true);
            event_sink::drain();
            let result = dispatch(
                "updateSetting",
                json!({"uploadPath":upload,"port":port2,"authEnable":false,"tusEnable":false,"chunkSize":20,"autoStart":false,"language":"zh","allowAnonymousUpload":true}),
                "",
            );
            assert_eq!(result["ok"], true);
            assert_eq!(server::status(), "start");
            assert!(std::net::TcpStream::connect(("127.0.0.1", port2)).is_ok());
            assert!(std::net::TcpListener::bind(("127.0.0.1", port1)).is_ok());
            let events = event_sink::drain();
            assert_eq!(
                events
                    .iter()
                    .map(|event| event["payload"]["status"].as_str().unwrap_or_default())
                    .collect::<Vec<_>>(),
                vec!["stop", "start"]
            );
            assert_eq!(
                setting::get_setting().unwrap()["allowAnonymousUpload"],
                true
            );
            server::stop().unwrap();
            config::set_plugin_dir(env!("CARGO_MANIFEST_DIR"));
            runtime::shutdown();
        });
    }

    #[test]
    fn dispatch_setting_methods() {
        crate::store::test_support::with_test_home(|home| {
            setting::update_ip(&json!("127.0.0.1")).unwrap();
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
                    "error": "更新上传路径失败，路径为空"
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
