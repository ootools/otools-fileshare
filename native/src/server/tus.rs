//! 可恢复分片上传：偏移取实际文件长度，metadata 跨页面刷新和插件重启保留。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{io::AsyncWriteExt, sync::Mutex};

use super::{api_error, routes_upload, AppState, Permissions};
use crate::{jwt, setting};

#[derive(Serialize, Deserialize)]
struct Session {
    name: String,
    length: u64,
    owner: String,
    #[serde(default)]
    completed: Option<PathBuf>,
    #[serde(default)]
    destination: Option<PathBuf>,
}

static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Weak<Mutex<()>>>>> = OnceLock::new();

async fn session_lock(id: &str) -> Arc<Mutex<()>> {
    let mut locks = LOCKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .await;
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(id).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(id.to_string(), Arc::downgrade(&lock));
    lock
}

fn owner(headers: &HeaderMap) -> String {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    jwt::verify(token)
        .and_then(|claims| {
            claims["username"]
                .as_str()
                .map(|name| format!("account:{name}"))
        })
        .unwrap_or_else(|| "local-or-anonymous".to_string())
}

fn protocol(headers: &HeaderMap) -> bool {
    headers
        .get("tus-resumable")
        .and_then(|value| value.to_str().ok())
        == Some("1.0.0")
}

async fn session_dir() -> Result<PathBuf, String> {
    let parent = routes_upload::upload_directory()?;
    let root = parent.join(".otools-fileshare-uploads");
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| error.to_string())?;
    let metadata = tokio::fs::symlink_metadata(&root)
        .await
        .map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("上传缓存目录不安全".to_string());
    }
    Ok(root)
}

async fn safe_open(path: &std::path::Path, append: bool) -> Result<tokio::fs::File, String> {
    let before = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|error| error.to_string())?;
    if !before.is_file() || before.file_type().is_symlink() {
        return Err("上传会话文件不安全".to_string());
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true).append(append);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    #[cfg(windows)]
    options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    let file = options
        .open(path)
        .await
        .map_err(|error| error.to_string())?;
    let metadata = file.metadata().await.map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("上传会话文件不是普通文件".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.ino() != metadata.ino() || before.dev() != metadata.dev() {
            return Err("上传会话文件已替换".to_string());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("上传会话文件为 reparse point".to_string());
        }
    }
    Ok(file)
}

async fn read_session(root: &std::path::Path, id: &str) -> Result<Session, Response> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err(StatusCode::NOT_FOUND.into_response());
    }
    let mut file = safe_open(&root.join(format!("{id}.json")), false)
        .await
        .map_err(|_| StatusCode::NOT_FOUND.into_response())?;
    let mut bytes = Vec::new();
    use tokio::io::AsyncReadExt;
    (&mut file)
        .take(64 * 1024)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| StatusCode::NOT_FOUND.into_response())?;
    serde_json::from_slice(&bytes).map_err(|_| StatusCode::NOT_FOUND.into_response())
}

async fn save_session(root: &std::path::Path, id: &str, session: &Session) -> Result<(), String> {
    let temporary =
        routes_upload::PendingFile(root.join(format!("{id}-{}.tmp", uuid::Uuid::new_v4())));
    tokio::fs::write(
        &temporary.0,
        serde_json::to_vec(session).map_err(|error| error.to_string())?,
    )
    .await
    .map_err(|error| error.to_string())?;
    tokio::fs::rename(&temporary.0, root.join(format!("{id}.json")))
        .await
        .map_err(|error| error.to_string())
}

async fn finalize_session(
    root: &std::path::Path,
    id: &str,
    session: &mut Session,
) -> Result<(), String> {
    let part = root.join(format!("{id}.part"));
    let file = safe_open(&part, false).await?;
    if file
        .metadata()
        .await
        .map_err(|error| error.to_string())?
        .len()
        != session.length
    {
        return Err("上传尚未完成".to_string());
    }
    drop(file);
    let directory = root.parent().ok_or("上传目录错误")?;
    routes_upload::safe_filename(&session.name)?;
    for suffix in 0..10000 {
        if session.destination.is_none() {
            let path = std::path::Path::new(&session.name);
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&session.name);
            let name = if suffix == 0 {
                session.name.clone()
            } else {
                match path.extension().and_then(|extension| extension.to_str()) {
                    Some(extension) => format!("{stem} ({suffix}).{extension}"),
                    None => format!("{stem} ({suffix})"),
                }
            };
            session.destination = Some(directory.join(name));
            save_session(root, id, session).await?; // 先记意图，再发布，崩溃后能识别目标。
        }
        let destination = session.destination.as_ref().unwrap().clone();
        if destination.parent() != Some(directory)
            || routes_upload::safe_filename(
                destination
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
            )
            .is_err()
        {
            return Err("目标路径无效".to_string());
        }
        match tokio::fs::hard_link(&part, &destination).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let source = tokio::fs::metadata(&part)
                    .await
                    .map_err(|error| error.to_string())?;
                let target = tokio::fs::symlink_metadata(&destination)
                    .await
                    .map_err(|error| error.to_string())?;
                #[cfg(unix)]
                let same = {
                    use std::os::unix::fs::MetadataExt;
                    !target.file_type().is_symlink()
                        && source.ino() == target.ino()
                        && source.dev() == target.dev()
                };
                #[cfg(windows)]
                let same = {
                    use std::os::windows::fs::MetadataExt;
                    target.is_file()
                        && target.file_attributes() & 0x400 == 0
                        && source.len() == target.len()
                        && source.creation_time() == target.creation_time()
                };
                if !same {
                    session.destination = None;
                    continue;
                }
            }
            Err(error) => return Err(error.to_string()),
        }
        crate::filedb::add_file(
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("文件名错误")?,
            destination.to_str().ok_or("文件路径错误")?,
            Some(&session.owner),
        )?;
        session.completed = Some(destination);
        save_session(root, id, session).await?; // completed 持久化前 part 始终保留。
        let _ = tokio::fs::remove_file(&part).await;
        return Ok(());
    }
    Err("同名文件过多".to_string())
}

fn tus_response(status: StatusCode, offset: u64, length: u64) -> Response {
    let mut response = status.into_response();
    let headers = response.headers_mut();
    headers.insert("tus-resumable", "1.0.0".parse().unwrap());
    headers.insert("upload-offset", offset.to_string().parse().unwrap());
    headers.insert("upload-length", length.to_string().parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

pub async fn create(
    Extension(permissions): Extension<Permissions>,
    headers: HeaderMap,
) -> Response {
    if !protocol(&headers) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    if let Err(error) = routes_upload::authorize_upload(&permissions.0, "uploadFile") {
        return api_error(403, error);
    }
    if setting::get_tus_enable()
        .ok()
        .and_then(|value| value.as_bool())
        != Some(true)
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let length = match headers
        .get("upload-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        Some(length) => length,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let metadata = headers
        .get("upload-metadata")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let filename = metadata.split(',').find_map(|entry| {
        entry
            .trim()
            .split_once(' ')
            .filter(|(key, _)| *key == "filename")
            .map(|(_, value)| value)
    });
    let filename = match filename
        .and_then(|value| STANDARD.decode(value).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
    {
        Some(filename) => filename,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    if routes_upload::safe_filename(&filename).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let root = match session_dir().await {
        Ok(root) => root,
        Err(error) => return api_error(500, error),
    };
    let id = uuid::Uuid::new_v4().to_string();
    let session = Session {
        name: filename,
        length,
        owner: owner(&headers),
        completed: None,
        destination: None,
    };
    if let Err(error) = tokio::fs::write(root.join(format!("{id}.part")), []).await {
        return api_error(500, error.to_string());
    }
    if let Err(error) = save_session(&root, &id, &session).await {
        let _ = tokio::fs::remove_file(root.join(format!("{id}.part"))).await;
        return api_error(500, error);
    }
    let mut response = tus_response(StatusCode::CREATED, 0, length);
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/uploads/{id}").parse().unwrap(),
    );
    response
}

pub async fn head(
    Path(id): Path<String>,
    Extension(permissions): Extension<Permissions>,
    headers: HeaderMap,
) -> Response {
    if !protocol(&headers) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    if let Err(error) = routes_upload::authorize_upload(&permissions.0, "uploadFile") {
        return api_error(403, error);
    }
    let lock = session_lock(&id).await;
    let _guard = lock.lock().await;
    let root = match session_dir().await {
        Ok(root) => root,
        Err(error) => return api_error(500, error),
    };
    let mut session = match read_session(&root, &id).await {
        Ok(session) => session,
        Err(response) => return response,
    };
    if session.owner != owner(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if session.completed.is_none() && session.destination.is_some() {
        if let Err(error) = finalize_session(&root, &id, &mut session).await {
            return api_error(500, error);
        }
    }
    let offset = if session.completed.is_some() {
        session.length
    } else {
        match safe_open(&root.join(format!("{id}.part")), false).await {
            Ok(file) => match file.metadata().await {
                Ok(metadata) => metadata.len(),
                Err(_) => return StatusCode::NOT_FOUND.into_response(),
            },
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        }
    };
    tus_response(StatusCode::OK, offset, session.length)
}

pub async fn patch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(permissions): Extension<Permissions>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if !protocol(&headers) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    if let Err(error) = routes_upload::authorize_upload(&permissions.0, "uploadFile") {
        return api_error(403, error);
    }
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        != Some("application/offset+octet-stream")
    {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let offset = match headers
        .get("upload-offset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        Some(offset) => offset,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let lock = session_lock(&id).await;
    let _guard = lock.lock().await;
    let root = match session_dir().await {
        Ok(root) => root,
        Err(error) => return api_error(500, error),
    };
    let mut session = match read_session(&root, &id).await {
        Ok(session) => session,
        Err(response) => return response,
    };
    if session.owner != owner(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if session.completed.is_some() {
        return tus_response(StatusCode::CONFLICT, session.length, session.length);
    }
    let temporary = root.join(format!("{id}.part"));
    let mut file = match safe_open(&temporary, true).await {
        Ok(file) => file,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let actual = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(error) => return api_error(500, error.to_string()),
    };
    if actual != offset {
        return tus_response(StatusCode::CONFLICT, actual, session.length);
    }
    let chunk_limit = setting::get_chunk_size()
        .ok()
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
        .unwrap_or(20)
        .saturating_mul(1024 * 1024);
    let mut stream = body.into_data_stream();
    let mut written = 0_u64;
    let mut stopping = state.stopping.clone();
    loop {
        let chunk = tokio::select! { _=stopping.changed()=>return StatusCode::SERVICE_UNAVAILABLE.into_response(),chunk=stream.next()=>chunk };
        match chunk {
            Some(Ok(chunk)) => {
                if (chunk.len() as u64) > chunk_limit.saturating_sub(written)
                    || (chunk.len() as u64) > session.length.saturating_sub(offset + written)
                {
                    return StatusCode::PAYLOAD_TOO_LARGE.into_response();
                }
                if let Err(error) = file.write_all(&chunk).await {
                    return api_error(500, error.to_string());
                }
                written += chunk.len() as u64;
            }
            Some(Err(error)) => return api_error(400, error.to_string()),
            None => break,
        }
    }
    if let Err(error) = file.flush().await {
        return api_error(500, error.to_string());
    }
    drop(file);
    let offset = offset + written;
    if offset == session.length {
        if let Err(error) = finalize_session(&root, &id, &mut session).await {
            return api_error(500, error);
        }
    }
    crate::event_sink::push(
        "fileshare:transfer.progress",
        json!({"name":session.name,"bytes":offset,"total":session.length,"direction":"upload","complete":session.completed.is_some()}),
    );
    tus_response(StatusCode::NO_CONTENT, offset, session.length)
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tower::ServiceExt;

    #[test]
    fn tus_head_resume_offset_conflict_and_completion() {
        with_test_home(|home| {
            let upload = home.join("uploads");
            std::fs::create_dir(&upload).unwrap();
            crate::setting::update_upload_path(&json!(upload)).unwrap();
            crate::setting::update_tus_enable(&json!(true)).unwrap();
            crate::store::set_item("allowAnonymousUpload", json!(true)).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = super::super::build_app(home.to_path_buf());
                let created = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri("/api/uploads")
                            .header("tus-resumable", "1.0.0")
                            .header("upload-length", "10")
                            .header("upload-metadata", "filename cmVzdW1lLnR4dA==")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(created.status(), StatusCode::CREATED);
                let url = created.headers()["location"].to_str().unwrap().to_string();
                let patch = |offset: &str, body: &'static str| {
                    Request::builder()
                        .method("PATCH")
                        .uri(&url)
                        .header("tus-resumable", "1.0.0")
                        .header("content-type", "application/offset+octet-stream")
                        .header("upload-offset", offset)
                        .body(Body::from(body))
                        .unwrap()
                };
                let first = app.clone().oneshot(patch("0", "01234")).await.unwrap();
                assert_eq!(first.status(), StatusCode::NO_CONTENT);
                assert_eq!(first.headers()["upload-offset"], "5");
                let conflict = app.clone().oneshot(patch("0", "56789")).await.unwrap();
                assert_eq!(conflict.status(), StatusCode::CONFLICT);
                let head = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method("HEAD")
                            .uri(&url)
                            .header("tus-resumable", "1.0.0")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(head.headers()["upload-offset"], "5");
                let final_chunk = app.clone().oneshot(patch("5", "56789")).await.unwrap();
                assert_eq!(final_chunk.status(), StatusCode::NO_CONTENT);
                assert_eq!(
                    std::fs::read(upload.join("resume.txt")).unwrap(),
                    b"0123456789"
                );
                assert_eq!(crate::filedb::list_files().unwrap().len(), 1);
                let body = to_bytes(final_chunk.into_body(), 1024).await.unwrap();
                assert!(body.is_empty());
            });
        });
    }
}
