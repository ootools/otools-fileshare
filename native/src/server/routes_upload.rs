//! 文件上传先鉴权，写临时文件后原子发布，禁止覆盖已有文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, Multipart, State},
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use super::{api_error, auth_enabled, routes_misc, AppState, Permissions};
use crate::{event_sink, filedb, setting};

pub fn authorize_upload(permissions: &Value, key: &str) -> Result<(), String> {
    if !auth_enabled()? && !routes_misc::anonymous_upload_enabled()? {
        return Err("匿名上传已关闭，请在本机开启或使用账号认证".to_string());
    }
    if permissions[key] != true {
        return Err(if key == "uploadText" {
            "无上传文本权限"
        } else {
            "无上传文件权限"
        }
        .to_string());
    }
    Ok(())
}

pub fn safe_filename(name: &str) -> Result<&str, String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.len() > 240
        || name.contains(['/', '\\', ':', '\0', '\r', '\n'])
        || name.ends_with(['.', ' '])
        || name.chars().any(|character| character.is_control())
        || name.starts_with(".otools-fileshare")
    {
        return Err("文件名称无效".to_string());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem[3..]
                .parse::<u8>()
                .is_ok_and(|number| (1..=9).contains(&number))
    {
        return Err("文件名称无效".to_string());
    }
    Ok(name)
}

pub fn upload_directory() -> Result<PathBuf, String> {
    let value = setting::get_upload_path()?;
    let directory = Path::new(value.as_str().ok_or("上传目录无效")?)
        .canonicalize()
        .map_err(|error| format!("上传目录不可用: {error}"))?;
    if !directory.is_dir() {
        return Err("上传路径必须为文件夹".to_string());
    }
    Ok(directory)
}

pub struct PendingFile(pub PathBuf);
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub async fn publish_file(
    temporary: &Path,
    directory: &Path,
    name: &str,
    username: Option<&str>,
) -> Result<PathBuf, String> {
    safe_filename(name)?;
    let source = Path::new(name);
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(name);
    let extension = source.extension().and_then(|extension| extension.to_str());
    for suffix in 0..10000 {
        let name = if suffix == 0 {
            name.to_string()
        } else {
            match extension {
                Some(extension) => format!("{stem} ({suffix}).{extension}"),
                None => format!("{stem} ({suffix})"),
            }
        };
        let destination = directory.join(&name);
        // hard_link 是同目录文件的原子 create-if-absent；不会覆盖、也不会跟随已有 symlink。
        match tokio::fs::hard_link(temporary, &destination).await {
            Ok(()) => {
                if let Err(error) = filedb::add_file(
                    &name,
                    destination.to_str().ok_or("文件路径编码无效")?,
                    username,
                ) {
                    let _ = tokio::fs::remove_file(&destination).await;
                    return Err(error);
                }
                let _ = tokio::fs::remove_file(temporary).await;
                return Ok(destination);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("同名文件过多".to_string())
}

pub async fn add_file(
    State(state): State<Arc<AppState>>,
    Extension(permissions): Extension<Permissions>,
    client: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    mut multipart: Multipart,
) -> Response {
    if let Err(error) = authorize_upload(&permissions.0, "uploadFile") {
        return api_error(403, error);
    }
    let directory = match upload_directory() {
        Ok(directory) => directory,
        Err(error) => return api_error(500, error),
    };
    let mut stopping = state.stopping.clone();
    while let Some(mut field) = match multipart.next_field().await {
        Ok(field) => field,
        Err(error) => return api_error(400, error.to_string()),
    } {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().unwrap_or_default().to_string();
        if let Err(error) = safe_filename(&name) {
            return api_error(400, error);
        }
        let temporary =
            PendingFile(directory.join(format!(".otools-fileshare-{}.part", uuid::Uuid::new_v4())));
        let mut file = match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary.0)
            .await
        {
            Ok(file) => file,
            Err(error) => return api_error(500, error.to_string()),
        };
        let mut transferred = 0_u64;
        loop {
            let chunk = tokio::select! {
                _=stopping.changed()=>return api_error(503,"服务已停止"),
                chunk=field.chunk()=>chunk,
            };
            match chunk {
                Ok(Some(bytes)) => {
                    if let Err(error) = file.write_all(&bytes).await {
                        return api_error(500, error.to_string());
                    }
                    transferred += bytes.len() as u64;
                }
                Ok(None) => break,
                Err(error) => return api_error(400, error.to_string()),
            }
        }
        if let Err(error) = file.flush().await {
            return api_error(500, error.to_string());
        }
        drop(file);
        let username = client.map(|client| client.0 .0.ip().to_string());
        return match publish_file(&temporary.0, &directory, &name, username.as_deref()).await {
            Ok(path) => {
                event_sink::push(
                    "fileshare:transfer.progress",
                    json!({"name":path.file_name().unwrap_or_default().to_string_lossy(),"bytes":transferred,"direction":"upload","complete":true}),
                );
                Json(json!({"code":200,"message":"添加成功"})).into_response()
            }
            Err(error) => api_error(500, error),
        };
    }
    api_error(400, "未收到上传文件")
}

pub async fn add_text(
    State(state): State<Arc<AppState>>,
    Extension(permissions): Extension<Permissions>,
    request: axum::extract::Request,
) -> Response {
    if let Err(error) = authorize_upload(&permissions.0, "uploadText") {
        return api_error(403, error);
    }
    let mut stopping = state.stopping.clone();
    if *stopping.borrow() {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let bytes = tokio::select! {
        biased;
        _=stopping.changed()=>return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
        bytes=axum::body::to_bytes(request.into_body(),2*1024*1024)=>match bytes {Ok(bytes)=>bytes,Err(error)=>return api_error(400,error.to_string())},
    };
    if *stopping.borrow() {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let input: Value = match serde_json::from_slice(&bytes) {
        Ok(input) => input,
        Err(error) => return api_error(400, error.to_string()),
    };
    let text = input["message"].as_str().unwrap_or_default();
    if text.is_empty() {
        return api_error(500, "消息不能为空");
    }
    match filedb::add_text(text, None) {
        Ok(_) => Json(json!({"code":200,"message":"添加成功"})).into_response(),
        Err(error) => api_error(500, error),
    }
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use futures_util::StreamExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    #[test]
    fn anonymous_upload_requires_opt_in_and_multipart_never_overwrites_existing_files() {
        with_test_home(|home| {
            let upload = home.join("uploads");
            std::fs::create_dir(&upload).unwrap();
            crate::setting::update_upload_path(&json!(upload)).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = super::super::build_app(home.to_path_buf());
                let request = || Request::builder().method("POST").uri("/api/addFile").header("content-type","multipart/form-data; boundary=test").body(Body::from("--test\r\nContent-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--test--\r\n")).unwrap();
                let denied = app.clone().oneshot(request()).await.unwrap();
                let denied:Value = serde_json::from_slice(&to_bytes(denied.into_body(),10000).await.unwrap()).unwrap();
                assert_eq!(denied["code"],403);
                assert_eq!(std::fs::read_dir(&upload).unwrap().count(),0);
                crate::store::set_item("allowAnonymousUpload",json!(true)).unwrap();
                for _ in 0..2 {
                    let response = app.clone().oneshot(request()).await.unwrap();
                    let value:Value = serde_json::from_slice(&to_bytes(response.into_body(),10000).await.unwrap()).unwrap();
                    assert_eq!(value["code"],200);
                }
                assert_eq!(std::fs::read(upload.join("hello.txt")).unwrap(),b"hello");
                assert_eq!(std::fs::read(upload.join("hello (1).txt")).unwrap(),b"hello");
                assert_eq!(crate::filedb::list_files().unwrap().len(),2);
                let traversal = app.oneshot(Request::builder().method("POST").uri("/api/addFile").header("content-type","multipart/form-data; boundary=test").body(Body::from("--test\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../outside.txt\"\r\n\r\nx\r\n--test--\r\n")).unwrap()).await.unwrap();
                let traversal:Value = serde_json::from_slice(&to_bytes(traversal.into_body(),10000).await.unwrap()).unwrap();
                assert_ne!(traversal["code"],200);
                assert!(!home.join("outside.txt").exists());
            });
        });
    }

    #[test]
    fn fifty_megabyte_multipart_stream_is_written_without_the_default_body_limit() {
        with_test_home(|home| {
            let upload = home.join("uploads");
            std::fs::create_dir(&upload).unwrap();
            crate::setting::update_upload_path(&json!(upload)).unwrap();
            crate::store::set_item("allowAnonymousUpload", json!(true)).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let stream = async_stream::stream! {
                    yield Ok::<_,std::io::Error>(axum::body::Bytes::from_static(b"--large\r\nContent-Disposition: form-data; name=\"file\"; filename=\"large.bin\"\r\n\r\n"));
                    for _ in 0..800 { yield Ok(axum::body::Bytes::from(vec![b'x';64*1024])); }
                    yield Ok(axum::body::Bytes::from_static(b"\r\n--large--\r\n"));
                };
                let app = super::super::build_app(home.to_path_buf());
                let response = app.oneshot(Request::builder().method("POST").uri("/api/addFile").header("content-type","multipart/form-data; boundary=large").body(Body::from_stream(stream)).unwrap()).await.unwrap();
                let value:Value = serde_json::from_slice(&to_bytes(response.into_body(),10000).await.unwrap()).unwrap();
                assert_eq!(value["code"],200);
                assert_eq!(std::fs::metadata(upload.join("large.bin")).unwrap().len(),50*1024*1024);
            });
        });
    }

    #[test]
    fn sse_initial_registry_and_list_changes_use_upstream_wire_format() {
        with_test_home(|home| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = super::super::build_app(home.to_path_buf());
                let response = app
                    .oneshot(
                        Request::builder()
                            .uri("/api/registrySSE")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let mut stream = response.into_body().into_data_stream();
                let first = stream.next().await.unwrap().unwrap();
                assert!(String::from_utf8_lossy(&first).contains("registry"));
                crate::filedb::add_text("hello", None).unwrap();
                let event = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                assert!(String::from_utf8_lossy(&event).contains("fileDb.listChange"));
            });
        });
    }
}
