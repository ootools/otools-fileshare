//! Setting 模块：上游 `shared/utils/Setting.js` 的 1:1 移植。
//!
//! 校验文案（'修改成功' / 'ValueNotChange' / '文件夹不存在' / ...）逐字保留；
//! 「值没变不更新」、updateUploadPath 缺 return 导致校验顺延等行为也按上游
//! 复刻，逐条在实现处注明。

use serde_json::{json, Value};
use std::sync::Mutex;

use crate::iputil;
use crate::store;

// ── K-V 键名（与上游 key 常量逐字一致）──────────────────────────────────────
pub const UPLOAD_PATH_KEY: &str = "uploadPath"; // 上传路径
pub const PORT_KEY: &str = "port"; // 端口号
pub const AUTH_ENABLE_KEY: &str = "authEnable"; // 是否开启密码校验
pub const TUS_ENABLE_KEY: &str = "tusEnable"; // 是否启用续传功能
pub const CHUNK_SIZE_KEY: &str = "chunkSize"; // 上传文件的分片大小
pub const AUTO_START_KEY: &str = "autoStart"; // 自动启动
pub const IP_FAMILY_KEY: &str = "ipFamily"; // IP协议族
pub const NET_INTERFACE_NAME_KEY: &str = "netInterfaceName"; // 网络接口名称
pub const LANGUAGE_KEY: &str = "language"; // 界面语言（zh / en）

// ── 上游校验文案（逐字）────────────────────────────────────────────────────
const MSG_MODIFIED: &str = "修改成功";
const MSG_VALUE_NOT_CHANGE: &str = "ValueNotChange";
const MSG_DIR_NOT_EXIST: &str = "文件夹不存在";
const MSG_UPLOAD_PATH_NOT_DIR: &str = "上传路径必须为文件夹";
const MSG_UPLOAD_PATH_EMPTY: &str = "更新上传路径失败，路径为空";
const MSG_PORT_EMPTY: &str = "更新端口失败，端口为空";
const MSG_VALUE_EMPTY: &str = "更新失败，值为空";
const MSG_CHUNK_EMPTY: &str = "更新分片大小失败，值为空";
const MSG_CHUNK_NOT_NUMBER: &str = "更新分片大小失败，值不是数字";
const MSG_CHUNK_NOT_POSITIVE: &str = "更新分片大小失败，值应该大于0";
const MSG_LANG_UNSUPPORTED: &str = "不支持的语言";
const MSG_IP_EMPTY: &str = "更新地址失败，地址为空";
const MSG_IP_FAMILY_EMPTY: &str = "IP协议族不能为空";
const MSG_IP_FAMILY_INVALID: &str = "IP协议族必须是ipv4或ipv6";
const MSG_NET_INTERFACE_EMPTY: &str = "网络接口名称不能为空";

/// updater 统一结果：`Ok(message)` = `{success:true,message}`，
/// `Err(message)` = `{success:false,message}`（reject 对象错误）。
pub type UpdateOutcome = Result<&'static str, String>;

/// 运行时 IP 覆盖（上游模块级变量 curIp）：仅本进程有效，不持久化。
static CUR_IP: Mutex<Option<String>> = Mutex::new(None);

// ── JS 语义辅助 ──────────────────────────────────────────────────────────────

/// JS 真值语义（上游大量使用 `!value` 判断）：null / false / 0 / NaN / 空串
/// 为假；空数组与空对象在 JS 中恒为真。
fn is_js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n
            .as_f64()
            .map(|f| f != 0.0 && !f.is_nan())
            .unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// JS 字符串上下文里的标量 → 字符串（模板串 `` `${v}` `` / String(v) 的近似）：
/// String 原样、Number 按 JS 规则输出、其余 JSON 文本化（面板只会发标量）。
fn value_to_js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

// ── 读取 ────────────────────────────────────────────────────────────────────

/// 上传路径默认值：上游 downloadsFolder() = 系统下载目录；取不到时回退
/// `~/Downloads`。
fn default_upload_path() -> String {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Downloads")))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn get_upload_path() -> Result<Value, String> {
    store::get_item_with_default(UPLOAD_PATH_KEY, Value::String(default_upload_path()))
}

pub fn get_port() -> Result<Value, String> {
    store::get_item_with_default(PORT_KEY, json!(5421))
}

pub fn get_auth_enable() -> Result<Value, String> {
    store::get_item_with_default(AUTH_ENABLE_KEY, json!(false))
}

pub fn get_tus_enable() -> Result<Value, String> {
    store::get_item_with_default(TUS_ENABLE_KEY, json!(false))
}

pub fn get_chunk_size() -> Result<Value, String> {
    store::get_item_with_default(CHUNK_SIZE_KEY, json!(20))
}

pub fn get_auto_start() -> Result<Value, String> {
    store::get_item_with_default(AUTO_START_KEY, json!(false))
}

/// 未设置时返回 null，由前端按系统语言决定默认值。
pub fn get_language() -> Result<Value, String> {
    store::get_item_with_default(LANGUAGE_KEY, Value::Null)
}

pub fn get_ip_family() -> Result<Value, String> {
    store::get_item_with_default(IP_FAMILY_KEY, json!("ipv4"))
}

pub fn get_net_interface_name() -> Result<Value, String> {
    store::get_item_with_default(NET_INTERFACE_NAME_KEY, json!(""))
}

/// 当前生效 IP：运行时覆盖 curIp 优先，否则走 IpUtil 回退。
pub fn get_ip() -> Result<String, String> {
    if let Some(ip) = CUR_IP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
    {
        return Ok(ip);
    }
    // TODO(Task 4): 接入真实 IpUtil（网卡枚举 + ipv6 加 []），当前桩恒回 127.0.0.1
    Ok(iputil::get_ip())
}

/// `http://<ip>:<port>`（上游 getUrl）。
pub fn get_url() -> Result<String, String> {
    let ip = get_ip()?;
    let port = get_port()?;
    Ok(format!("http://{}:{}", ip, value_to_js_string(&port)))
}

/// 聚合配置（键与上游 getSetting 完全一致：不含 ipFamily / netInterfaceName）。
pub fn get_setting() -> Result<Value, String> {
    let port = get_port()?;
    let ip = get_ip()?;
    Ok(json!({
        "uploadPath": get_upload_path()?,
        "port": port,
        "ip": ip,
        "url": format!("http://{}:{}", ip, value_to_js_string(&port)),
        "authEnable": get_auth_enable()?,
        "tusEnable": get_tus_enable()?,
        "chunkSize": get_chunk_size()?,
        "autoStart": get_auto_start()?,
        "language": get_language()?,
    }))
}

// ── 更新 ────────────────────────────────────────────────────────────────────

pub fn update_upload_path(path: &Value) -> UpdateOutcome {
    // 上游 !path：空串 / null / 0 等假值直接 reject
    if !is_js_truthy(path) {
        return Err(MSG_UPLOAD_PATH_EMPTY.to_string());
    }
    // 上游比较 getUploadPath() === path
    let unchanged = get_upload_path()? == *path;
    // 上游 executor 里 resolve(ValueNotChange) 同步先 settle：JS Promise 首个
    // settle 生效，后续 reject 全是 no-op——即「值未变」时无论目录是否存在/
    // 是否目录，结果都是 ValueNotChange（setStorageItem 是同值幂等写入）。
    if unchanged {
        store::set_item(UPLOAD_PATH_KEY, path.clone())?;
        return Ok(MSG_VALUE_NOT_CHANGE);
    }
    let path_str = value_to_js_string(path);
    let target = std::path::Path::new(&path_str);
    // 上游 fs.existsSync（跟随符号链接）
    if !target.exists() {
        return Err(MSG_DIR_NOT_EXIST.to_string());
    }
    // 上游 fs.lstatSync().isDirectory()：不跟随符号链接（指向目录的符号链接
    // 同样会被判为「必须为文件夹」，按上游行为保留）。
    let metadata = std::fs::symlink_metadata(&path_str).map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err(MSG_UPLOAD_PATH_NOT_DIR.to_string());
    }
    store::set_item(UPLOAD_PATH_KEY, path.clone())?;
    Ok(MSG_MODIFIED)
}

pub fn update_port(port: &Value) -> UpdateOutcome {
    if !is_js_truthy(port) {
        return Err(MSG_PORT_EMPTY.to_string());
    }
    // 上游这里 resolve 后有 return：值没变直接返回，不再做别的校验
    if get_port()? == *port {
        return Ok(MSG_VALUE_NOT_CHANGE);
    }
    store::set_item(PORT_KEY, port.clone())?;
    Ok(MSG_MODIFIED)
}

/// updateAuthEnable / updateTusEnable / updateAutoStart 共用形状：上游仅
/// `value == null` 才 reject（false 是合法值），写入后恒回「修改成功」。
fn update_bool_flag(key: &str, value: &Value) -> UpdateOutcome {
    // JS `value == null`：仅 null / undefined（payload 缺键即 null）
    if value.is_null() {
        return Err(MSG_VALUE_EMPTY.to_string());
    }
    store::set_item(key, value.clone())?;
    Ok(MSG_MODIFIED)
}

pub fn update_auth_enable(value: &Value) -> UpdateOutcome {
    update_bool_flag(AUTH_ENABLE_KEY, value)
}

pub fn update_tus_enable(value: &Value) -> UpdateOutcome {
    update_bool_flag(TUS_ENABLE_KEY, value)
}

pub fn update_auto_start(value: &Value) -> UpdateOutcome {
    update_bool_flag(AUTO_START_KEY, value)
}

pub fn update_chunk_size(chunk_size: &Value) -> UpdateOutcome {
    if !is_js_truthy(chunk_size) {
        return Err(MSG_CHUNK_EMPTY.to_string());
    }
    // 上游 isNumber：number 直接算；string 要求非空且能转数字。
    // JS Number("  ") === 0（空白串转 0），对齐成「>0 校验会拦下它」。
    let numeric = match chunk_size {
        Value::Number(_) => chunk_size.as_f64(),
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Some(0.0)
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    };
    let Some(value) = numeric else {
        return Err(MSG_CHUNK_NOT_NUMBER.to_string());
    };
    if value <= 0.0 {
        return Err(MSG_CHUNK_NOT_POSITIVE.to_string());
    }
    // 上游严格 === 比较：字符串 "30" 与数字 30 视为不同值
    if get_chunk_size()? == *chunk_size {
        return Ok(MSG_VALUE_NOT_CHANGE);
    }
    // 上游按原类型存储（字符串数字存的就是字符串）
    store::set_item(CHUNK_SIZE_KEY, chunk_size.clone())?;
    Ok(MSG_MODIFIED)
}

pub fn update_language(value: &Value) -> UpdateOutcome {
    // 上游：null / undefined / 空串 = 「不修改」，resolve（非 reject）
    match value {
        Value::Null => Ok(MSG_VALUE_NOT_CHANGE),
        Value::String(s) if s.is_empty() => Ok(MSG_VALUE_NOT_CHANGE),
        Value::String(_) => {
            // 仅 zh / en（严格比较，区分大小写）
            if value.as_str() != Some("zh") && value.as_str() != Some("en") {
                return Err(MSG_LANG_UNSUPPORTED.to_string());
            }
            if get_language()? == *value {
                return Ok(MSG_VALUE_NOT_CHANGE);
            }
            store::set_item(LANGUAGE_KEY, value.clone())?;
            Ok(MSG_MODIFIED)
        }
        // 非字符串真值：!== 'zh' && !== 'en' → 不支持的语言
        _ => Err(MSG_LANG_UNSUPPORTED.to_string()),
    }
}

/// 运行时 IP 覆盖，不持久化（上游 updateIp）。
pub fn update_ip(ip: &Value) -> UpdateOutcome {
    let ip_str = match ip {
        Value::String(s) if !s.is_empty() => s.clone(),
        // 面板只发字符串；其他真值按 JSON 文本化兜底（上游会存原始类型）
        other if is_js_truthy(other) => value_to_js_string(other),
        _ => return Err(MSG_IP_EMPTY.to_string()),
    };
    // 上游比较 getIp() === ip（curIp 覆盖优先，否则 IpUtil 回退值）
    if get_ip()? == ip_str {
        return Ok(MSG_VALUE_NOT_CHANGE);
    }
    *CUR_IP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ip_str);
    Ok(MSG_MODIFIED)
}

/// 逐键走对应 updater，顺序与上游 updateSetting 一致。
///
/// Promise.all 语义：7 个 updater 全部执行完（副作用照常发生），整体
/// reject 时报顺序内第一个失败。缺键按 null 处理，对应上游 `undefined`。
pub fn update_setting(setting: &Value) -> UpdateOutcome {
    let get = |key: &str| setting.get(key).cloned().unwrap_or(Value::Null);
    let results = [
        update_upload_path(&get(UPLOAD_PATH_KEY)),
        update_port(&get(PORT_KEY)),
        update_auth_enable(&get(AUTH_ENABLE_KEY)),
        update_tus_enable(&get(TUS_ENABLE_KEY)),
        update_chunk_size(&get(CHUNK_SIZE_KEY)),
        update_auto_start(&get(AUTO_START_KEY)),
        update_language(&get(LANGUAGE_KEY)),
    ];
    if let Some(Err(message)) = results.into_iter().find(|r| r.is_err()) {
        return Err(message);
    }
    Ok(MSG_MODIFIED)
}

// ── 同步 setter（上游直接 throw；Task 4 的 IpUtil 包装后作方法级 reject）────

pub fn set_ip_family(value: &Value) -> Result<(), String> {
    // 上游 setIpFamily 直接 throw（同步），Task 4 的 IpUtil.setIpFamily 包装后
    // 才作为方法级 reject 暴露。
    if !is_js_truthy(value) {
        return Err(MSG_IP_FAMILY_EMPTY.to_string());
    }
    let Some(family) = value.as_str() else {
        // 非字符串真值：!== 'ipv4' && !== 'ipv6' → 非法
        return Err(MSG_IP_FAMILY_INVALID.to_string());
    };
    if family != "ipv4" && family != "ipv6" {
        return Err(MSG_IP_FAMILY_INVALID.to_string());
    }
    store::set_item(IP_FAMILY_KEY, value.clone())
}

pub fn set_net_interface_name(value: &Value) -> Result<(), String> {
    // 上游 !value：空串也 reject（上游没有提供清空的路径）
    if !is_js_truthy(value) {
        return Err(MSG_NET_INTERFACE_EMPTY.to_string());
    }
    store::set_item(NET_INTERFACE_NAME_KEY, value.clone())
}

// ── wire 装配 ───────────────────────────────────────────────────────────────

/// updater 结果 → wire data：`Ok(message)` → `{"success":true,"message":…}`；
/// `Err(message)` 由 dispatch 层包成 `{"ok":false,"error":{"success":false,"message":…}}`。
pub fn update_outcome_to_data(outcome: UpdateOutcome) -> Result<Value, String> {
    outcome.map(|message| json!({ "success": true, "message": message }))
}

/// 测试隔离：清掉运行时 curIp 覆盖（store::test_support::with_test_home 调用）。
#[cfg(test)]
pub(crate) fn test_reset_cur_ip() {
    *CUR_IP
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::with_test_home;

    #[test]
    fn port_defaults_to_5421() {
        with_test_home(|_| {
            assert_eq!(get_port().unwrap(), json!(5421));
        });
    }

    #[test]
    fn update_port_empty_rejects() {
        with_test_home(|_| {
            assert_eq!(
                update_port(&Value::Null).unwrap_err(),
                "更新端口失败，端口为空"
            );
            assert_eq!(
                update_port(&json!("")).unwrap_err(),
                "更新端口失败，端口为空"
            );
            assert_eq!(
                update_port(&json!(0)).unwrap_err(),
                "更新端口失败，端口为空"
            );
        });
    }

    #[test]
    fn update_port_roundtrip_and_value_not_change() {
        with_test_home(|_| {
            assert_eq!(update_port(&json!(6000)).unwrap(), "修改成功");
            assert_eq!(get_port().unwrap(), json!(6000));
            assert_eq!(update_port(&json!(6000)).unwrap(), "ValueNotChange");
        });
    }

    #[test]
    fn language_only_zh_en() {
        with_test_home(|_| {
            // 未设置 = null；空值 = 「不修改」，resolve（非 reject）
            assert_eq!(get_language().unwrap(), Value::Null);
            assert_eq!(update_language(&Value::Null).unwrap(), "ValueNotChange");
            assert_eq!(update_language(&json!("")).unwrap(), "ValueNotChange");
            // 仅 zh / en（区分大小写，与上游严格比较一致）
            assert_eq!(update_language(&json!("fr")).unwrap_err(), "不支持的语言");
            assert_eq!(update_language(&json!("ZH")).unwrap_err(), "不支持的语言");
            assert_eq!(update_language(&json!("zh")).unwrap(), "修改成功");
            assert_eq!(get_language().unwrap(), json!("zh"));
            assert_eq!(update_language(&json!("zh")).unwrap(), "ValueNotChange");
            assert_eq!(update_language(&json!("en")).unwrap(), "修改成功");
            assert_eq!(get_language().unwrap(), json!("en"));
        });
    }

    #[test]
    fn upload_path_validation_messages() {
        with_test_home(|home| {
            // 空值
            assert_eq!(
                update_upload_path(&Value::Null).unwrap_err(),
                "更新上传路径失败，路径为空"
            );
            assert_eq!(
                update_upload_path(&json!("")).unwrap_err(),
                "更新上传路径失败，路径为空"
            );
            // 不存在
            assert_eq!(
                update_upload_path(&json!(home.join("nope")))
                    .unwrap_err(),
                "文件夹不存在"
            );
            // 指向文件
            let file = home.join("afile.txt");
            std::fs::write(&file, b"x").unwrap();
            assert_eq!(
                update_upload_path(&json!(file)).unwrap_err(),
                "上传路径必须为文件夹"
            );
            // 合法目录 → 修改成功；再来一次 → ValueNotChange
            let dir = home.join("updir");
            std::fs::create_dir_all(&dir).unwrap();
            assert_eq!(update_upload_path(&json!(dir)).unwrap(), "修改成功");
            assert_eq!(get_upload_path().unwrap(), json!(dir));
            assert_eq!(update_upload_path(&json!(dir)).unwrap(), "ValueNotChange");
            // 值未变但目录已被删：上游 JS Promise 首个 settle 生效 → 仍是
            // ValueNotChange（后续 reject 是 no-op），不能变成「文件夹不存在」
            std::fs::remove_dir_all(&dir).unwrap();
            assert_eq!(update_upload_path(&json!(dir)).unwrap(), "ValueNotChange");
        });
    }

    #[test]
    fn auth_enable_defaults_false_and_null_rejects() {
        with_test_home(|_| {
            assert_eq!(get_auth_enable().unwrap(), json!(false));
            // false 是合法值（上游仅 value == null 才 reject）
            assert_eq!(update_auth_enable(&json!(false)).unwrap(), "修改成功");
            assert_eq!(get_auth_enable().unwrap(), json!(false));
            assert_eq!(update_auth_enable(&json!(true)).unwrap(), "修改成功");
            assert_eq!(get_auth_enable().unwrap(), json!(true));
            assert_eq!(
                update_auth_enable(&Value::Null).unwrap_err(),
                "更新失败，值为空"
            );
        });
    }

    #[test]
    fn get_setting_shape_and_url_follows_port() {
        with_test_home(|_| {
            let s = get_setting().unwrap();
            for key in [
                "uploadPath",
                "port",
                "ip",
                "url",
                "authEnable",
                "tusEnable",
                "chunkSize",
                "autoStart",
                "language",
            ] {
                assert!(s.get(key).is_some(), "get_setting 缺键 {key}");
            }
            assert_eq!(s["port"], json!(5421));
            assert_eq!(s["authEnable"], json!(false));
            assert_eq!(s["tusEnable"], json!(false));
            assert_eq!(s["chunkSize"], json!(20));
            assert_eq!(s["autoStart"], json!(false));
            assert_eq!(s["language"], Value::Null);
            // ip 桩 = 127.0.0.1（TODO Task 4 接真实 IpUtil）
            assert_eq!(s["ip"], json!("127.0.0.1"));
            assert_eq!(s["url"], json!("http://127.0.0.1:5421"));
            // 改端口后 url 跟随
            update_port(&json!(6000)).unwrap();
            assert_eq!(get_setting().unwrap()["url"], json!("http://127.0.0.1:6000"));
        });
    }

    #[test]
    fn chunk_size_validation() {
        with_test_home(|_| {
            assert_eq!(get_chunk_size().unwrap(), json!(20));
            assert_eq!(
                update_chunk_size(&json!(0)).unwrap_err(),
                "更新分片大小失败，值为空"
            );
            assert_eq!(
                update_chunk_size(&json!("")).unwrap_err(),
                "更新分片大小失败，值为空"
            );
            assert_eq!(
                update_chunk_size(&json!("abc")).unwrap_err(),
                "更新分片大小失败，值不是数字"
            );
            assert_eq!(
                update_chunk_size(&json!(-5)).unwrap_err(),
                "更新分片大小失败，值应该大于0"
            );
            // 字符串数字按原类型存储（上游严格 === 比较）
            assert_eq!(update_chunk_size(&json!("30")).unwrap(), "修改成功");
            assert_eq!(get_chunk_size().unwrap(), json!("30"));
            assert_eq!(update_chunk_size(&json!("30")).unwrap(), "ValueNotChange");
        });
    }

    #[test]
    fn tus_enable_and_auto_start_flags() {
        with_test_home(|_| {
            assert_eq!(get_tus_enable().unwrap(), json!(false));
            assert_eq!(get_auto_start().unwrap(), json!(false));
            assert_eq!(
                update_tus_enable(&Value::Null).unwrap_err(),
                "更新失败，值为空"
            );
            assert_eq!(
                update_auto_start(&Value::Null).unwrap_err(),
                "更新失败，值为空"
            );
            assert_eq!(update_auto_start(&json!(true)).unwrap(), "修改成功");
            assert_eq!(get_auto_start().unwrap(), json!(true));
        });
    }

    #[test]
    fn update_setting_applies_all_and_reports_first_error() {
        with_test_home(|home| {
            let dir = home.join("up");
            std::fs::create_dir_all(&dir).unwrap();
            // 全部合法 → 成功且逐键生效
            let ok = json!({
                "uploadPath": dir.to_string_lossy(),
                "port": 7000,
                "authEnable": true,
                "tusEnable": false,
                "chunkSize": 20,
                "autoStart": false,
                "language": "en",
            });
            assert_eq!(update_setting(&ok).unwrap(), "修改成功");
            let s = get_setting().unwrap();
            assert_eq!(s["port"], json!(7000));
            assert_eq!(s["authEnable"], json!(true));
            assert_eq!(s["language"], json!("en"));
            assert_eq!(s["uploadPath"], json!(dir.to_string_lossy()));

            // Promise.all 语义：非法端口 + 合法 authEnable —— 全部副作用发生，
            // 整体 reject 且报顺序内第一个错误（前面键都合法，端口是第一个错）
            let partial = json!({
                "uploadPath": dir.to_string_lossy(),
                "port": 0,
                "authEnable": false,
            });
            assert_eq!(
                update_setting(&partial).unwrap_err(),
                "更新端口失败，端口为空"
            );
            let s = get_setting().unwrap();
            assert_eq!(
                s["authEnable"],
                json!(false),
                "Promise.all 副作用应已发生"
            );
            assert_eq!(s["port"], json!(7000), "非法值不落盘");
        });
    }

    #[test]
    fn update_setting_missing_keys_reject_like_upstream() {
        with_test_home(|_| {
            // 上游 setting[uploadPathKey] 为 undefined → 第一个 updater 就 reject
            assert_eq!(
                update_setting(&json!({ "port": 7777 })).unwrap_err(),
                "更新上传路径失败，路径为空"
            );
            // 但端口已按 Promise.all 副作用写入
            assert_eq!(get_port().unwrap(), json!(7777));
        });
    }

    #[test]
    fn update_ip_runtime_only() {
        with_test_home(|_| {
            assert_eq!(
                update_ip(&Value::Null).unwrap_err(),
                "更新地址失败，地址为空"
            );
            assert_eq!(update_ip(&json!("")).unwrap_err(), "更新地址失败，地址为空");
            // 与 IpUtil 桩回退值相同 → ValueNotChange
            assert_eq!(update_ip(&json!("127.0.0.1")).unwrap(), "ValueNotChange");
            assert_eq!(update_ip(&json!("192.168.1.5")).unwrap(), "修改成功");
            assert_eq!(get_ip().unwrap(), "192.168.1.5");
            assert_eq!(get_url().unwrap(), "http://192.168.1.5:5421");
            // 运行时覆盖不持久化：重置后回到回退值
            test_reset_cur_ip();
            assert_eq!(get_ip().unwrap(), "127.0.0.1");
        });
    }

    #[test]
    fn ip_family_validation() {
        with_test_home(|_| {
            assert_eq!(get_ip_family().unwrap(), json!("ipv4"));
            assert_eq!(
                set_ip_family(&Value::Null).unwrap_err(),
                "IP协议族不能为空"
            );
            assert_eq!(
                set_ip_family(&json!("ipv5")).unwrap_err(),
                "IP协议族必须是ipv4或ipv6"
            );
            assert!(set_ip_family(&json!("ipv6")).is_ok());
            assert_eq!(get_ip_family().unwrap(), json!("ipv6"));
        });
    }

    #[test]
    fn net_interface_name_validation() {
        with_test_home(|_| {
            assert_eq!(get_net_interface_name().unwrap(), json!(""));
            assert_eq!(
                set_net_interface_name(&json!("")).unwrap_err(),
                "网络接口名称不能为空"
            );
            assert!(set_net_interface_name(&json!("en0")).is_ok());
            assert_eq!(get_net_interface_name().unwrap(), json!("en0"));
        });
    }
}
