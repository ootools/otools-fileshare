//! 与上游手机页面保持相同的认证、设置响应形状。

use axum::{
    body::to_bytes,
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde_json::{json, Value};

use super::{api_error, auth_enabled, Permissions};
use crate::{account, jwt, setting, store};

pub async fn login(request: Request) -> Response {
    let enabled = match auth_enabled() {
        Ok(value) => value,
        Err(error) => return api_error(500, error),
    };
    if !enabled {
        let permissions = jwt::admin_permissions();
        // 匿名访问不签发任何可在开启认证后复用的凭证。
        return Json(json!({"code":200,"data":{"Authorization":"","permissions":permissions},"message":"success"})).into_response();
    }
    let content_type = request
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = match to_bytes(request.into_body(), 64 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let input: Value = if content_type.starts_with("application/x-www-form-urlencoded") {
        match serde_urlencoded::from_bytes::<std::collections::HashMap<String, String>>(&bytes) {
            Ok(fields) => json!(fields),
            Err(_) => return api_error(403, "登录参数无效"),
        }
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => return api_error(403, "登录参数无效"),
        }
    };
    let username = input["username"].as_str().unwrap_or_default().trim();
    if username.is_empty() {
        return api_error(403, "请输入用户名");
    }
    let account = match account::get_account_by_username(username) {
        Ok(Some(account)) => account,
        Ok(None) => return api_error(403, "用户名或密码错误"),
        Err(error) => return api_error(500, error),
    };
    if input["password"].as_str().is_none() || input["password"] != account["password"] {
        return api_error(403, "用户名或密码错误");
    }
    let permissions = account["permissions"].clone();
    Json(json!({"code":200,"data":{"Authorization":jwt::sign(json!({"username":username,"permissions":permissions}),0),"permissions":permissions},"message":"success"})).into_response()
}

pub async fn get_setting(Extension(permissions): Extension<Permissions>) -> Response {
    match auth_enabled() {
        Ok(auth_enable) => {
            let mut permissions = permissions.0;
            if !auth_enable && !anonymous_upload_enabled().unwrap_or(false) {
                permissions["uploadFile"] = json!(false);
                permissions["uploadText"] = json!(false);
            }
            Json(json!({"code":200,"data":{"authEnable":auth_enable,"permissions":permissions},"message":"success"})).into_response()
        }
        Err(error) => api_error(500, error),
    }
}

pub async fn tus_config() -> Response {
    let values = (setting::get_tus_enable(), setting::get_chunk_size());
    match values {
        (Ok(enable), Ok(chunk_size)) => {
            Json(json!({"code":200,"data":{"enable":enable,"chunkSize":chunk_size}}))
                .into_response()
        }
        _ => api_error(500, "无法读取上传配置"),
    }
}

pub fn anonymous_upload_enabled() -> Result<bool, String> {
    Ok(store::get_item("allowAnonymousUpload")?
        .and_then(|value| value.as_bool())
        .unwrap_or(false))
}

pub async fn logout() -> Json<Value> {
    Json(json!({"code":200,"message":"success"}))
}
