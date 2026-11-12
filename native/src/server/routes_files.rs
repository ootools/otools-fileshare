use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Deserialize;
use serde_json::json;

use super::{api_error, fileutil, Permissions};
use crate::{account, filedb};

#[derive(Deserialize)]
pub struct FilesQuery {
    #[serde(default)]
    path: String,
}

pub async fn list_files(
    Query(query): Query<FilesQuery>,
    Extension(permissions): Extension<Permissions>,
) -> Response {
    let (path, files) = if query.path.trim_matches('/').is_empty() {
        match filedb::list_files() {
            Ok(files) => (Vec::new(), files),
            Err(error) => return api_error(500, error),
        }
    } else {
        let (resolved, segments) = match fileutil::parse_path(&query.path) {
            Ok(result) => result,
            Err(error) if error == "分享列表未找到该文件" => {
                return api_error(500, error)
            }
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        };
        if !account::can_access_path(&resolved.to_string_lossy(), &permissions.0) {
            return StatusCode::FORBIDDEN.into_response();
        }
        match fileutil::list_dir(&resolved) {
            Ok(files) => (segments, files),
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        }
    };
    let files: Vec<_> = files
        .into_iter()
        .filter(|file| {
            let Some(path) = file["path"].as_str() else {
                return true;
            };
            if !account::can_access_path(path, &permissions.0) {
                return false;
            }
            // 目录中的符号链接不得借列表泄露共享范围外条目。
            if !query.path.trim_matches('/').is_empty() {
                let key = format!(
                    "{}/{}",
                    query.path.trim_matches('/'),
                    file["name"].as_str().unwrap_or_default()
                );
                if fileutil::parse_path(&key).is_err() {
                    return false;
                }
            }
            true
        })
        .collect();
    Json(json!({"code":200,"data":{"path":path,"files":files}})).into_response()
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    #[test]
    fn login_listing_permissions_and_share_path_confinement() {
        with_test_home(|home| {
            let first = home.join("first/photos");
            let second = home.join("second/photos");
            std::fs::create_dir_all(&first).unwrap();
            std::fs::create_dir_all(&second).unwrap();
            std::fs::write(second.join("pic.txt"), "picture").unwrap();
            crate::filedb::add_file("photos", first.to_str().unwrap(), None).unwrap();
            crate::filedb::add_file("photos", second.to_str().unwrap(), None).unwrap();
            assert_eq!(
                super::super::fileutil::parse_path("photos_1/pic.txt")
                    .unwrap()
                    .0,
                second.join("pic.txt").canonicalize().unwrap()
            );
            assert!(super::super::fileutil::parse_path("photos_1/../../outside").is_err());
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(home, second.join("escape")).unwrap();
                assert!(super::super::fileutil::parse_path("photos_1/escape").is_err());
            }
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = super::super::build_app(home.to_path_buf());
                let listed = app.clone().oneshot(Request::builder().uri("/api/files?path=photos_1").body(Body::empty()).unwrap()).await.unwrap();
                assert_eq!(listed.status(),StatusCode::OK);
                let listed: Value = serde_json::from_slice(&to_bytes(listed.into_body(), 10000).await.unwrap()).unwrap();
                assert_eq!(listed["code"],200);
                assert!(listed["data"]["files"].as_array().unwrap().iter().any(|file| file["name"] == "pic.txt"));
                crate::account::add_account(&json!({"username":"alice","password":"secret","permissions":{"download":false,"denyAccess":second.canonicalize().unwrap().to_string_lossy()}})).unwrap();
                crate::setting::update_auth_enable(&json!(true)).unwrap();
                let login = app.clone().oneshot(Request::builder().method("POST").uri("/api/login").header("content-type","application/json").body(Body::from(r#"{"username":"alice","password":"secret"}"#)).unwrap()).await.unwrap();
                let login: Value = serde_json::from_slice(&to_bytes(login.into_body(),10000).await.unwrap()).unwrap();
                assert_eq!(login["code"],200);
                assert_eq!(login["data"]["permissions"]["download"],false);
                let token = login["data"]["Authorization"].as_str().unwrap();
                let denied = app.clone().oneshot(Request::builder().uri("/api/files?path=photos_1").header("authorization",token).body(Body::empty()).unwrap()).await.unwrap();
                assert_eq!(denied.status(),StatusCode::FORBIDDEN);
                let config = app.oneshot(Request::builder().uri("/api/tusConfig").header("authorization",token).body(Body::empty()).unwrap()).await.unwrap();
                let config: Value = serde_json::from_slice(&to_bytes(config.into_body(),10000).await.unwrap()).unwrap();
                assert_eq!(config["data"]["enable"],false);
                assert_eq!(config["data"]["chunkSize"],20);
            });
        });
    }
}
