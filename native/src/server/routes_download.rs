//! 流式下载，Range 与中文文件名保持浏览器及多线程下载器兼容。

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use super::{auth_enabled, fileutil, AppState};
use crate::{account, jwt, ziputil};

#[derive(Deserialize)]
pub struct DownloadQuery {
    #[serde(default)]
    filename: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    timestamp: String,
}

static ARCHIVE_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn byte_range(range: Option<&str>, size: u64) -> Result<(u64, u64, bool), ()> {
    let Some(range) = range else {
        return Ok((0, size, false));
    };
    let range = range.strip_prefix("bytes=").ok_or(())?;
    if range.contains(',') || size == 0 {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;
    if start.is_empty() {
        let count = end.parse::<u64>().map_err(|_| ())?;
        if count == 0 {
            return Err(());
        }
        let start = size.saturating_sub(count);
        return Ok((start, size - start, true));
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= size {
        return Err(());
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(size - 1)
    };
    if end < start {
        return Err(());
    }
    Ok((start, end - start + 1, true))
}

async fn archive_path(
    path: &Path,
    permissions: &Value,
    timestamp: &str,
    state: &AppState,
) -> Result<PathBuf, String> {
    let _guard = ARCHIVE_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    if *state.stopping.borrow() {
        return Err("服务已停止".to_string());
    }
    let root = fileutil::temp_dir().join("archives");
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| error.to_string())?;
    // 缓存包含权限指纹，账号修改后不能命中旧的、权限更宽的 ZIP。
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| error.to_string())?;
    let fingerprint = format!(
        "{}:{permissions}:{timestamp}:{:?}",
        path.display(),
        metadata.modified().ok()
    );
    let key = format!("{:x}", Sha256::digest(fingerprint.as_bytes()));
    let output = root.join(format!("{key}.zip"));
    if output.is_file() {
        return Ok(output);
    }
    // 只清理本插件缓存里的旧 zip，不触碰任意用户目录。
    if let Ok(mut entries) = tokio::fs::read_dir(&root).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "zip") {
                if let Ok(metadata) = entry.metadata().await {
                    if metadata
                        .modified()
                        .ok()
                        .and_then(|time| SystemTime::now().duration_since(time).ok())
                        .is_some_and(|age| age > Duration::from_secs(1800))
                    {
                        let _ = tokio::fs::remove_file(entry.path()).await;
                    }
                }
            }
        }
    }
    let source = path.to_path_buf();
    let temporary = root.join(format!("{}.partial", uuid::Uuid::new_v4()));
    let target = temporary.clone();
    let permissions = permissions.clone();
    let stopping = state.stopping.clone();
    let result = tokio::task::spawn_blocking(move || {
        ziputil::zip_directory_with_cancel(&source, &target, &permissions, Some(stopping))
    })
    .await
    .map_err(|error| error.to_string())?;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error);
    }
    tokio::fs::rename(&temporary, &output)
        .await
        .map_err(|error| error.to_string())?;
    Ok(output)
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Response {
    let permissions = match auth_enabled() {
        Ok(false) => jwt::admin_permissions(),
        Ok(true) => match jwt::verify(&query.token)
            .and_then(|payload| jwt::resolve_permissions(&payload).ok().flatten())
        {
            Some(permissions) if permissions["download"] == true => permissions,
            _ => return StatusCode::FORBIDDEN.into_response(),
        },
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if query.filename.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let (path, _) = match fileutil::parse_path(&query.filename) {
        Ok(value) => value,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if !account::can_access_path(&path.to_string_lossy(), &permissions) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let (path, name) = if path.is_dir() {
        match archive_path(&path, &permissions, &query.timestamp, &state).await {
            Ok(archive) => (archive, format!("{name}.zip")),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    } else {
        (path, name)
    };
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let size = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let (start, length, partial) = match byte_range(
        headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        size,
    ) {
        Ok(range) => range,
        Err(_) => {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response()
        }
    };
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let mut reader = ReaderStream::with_capacity(file.take(length), 64 * 1024);
    let mut stopping = state.stopping.clone();
    let stream = async_stream::stream! {
        // 响应体比 handler/Router 活得更久，保留发送端直到下载结束。
        let _state = state;
        use futures_util::StreamExt;
        loop {
            tokio::select! {
                _ = stopping.changed() => break,
                chunk = reader.next() => match chunk { Some(chunk) => yield chunk, None => break },
            }
        }
    };
    let mut response = Body::from_stream(stream).into_response();
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let response_headers = response.headers_mut();
    response_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    response_headers.insert(header::CONTENT_LENGTH, length.to_string().parse().unwrap());
    response_headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    let encoded = utf8_percent_encode(&name, NON_ALPHANUMERIC).to_string();
    response_headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"download\"; filename*=UTF-8''{encoded}")
            .parse()
            .unwrap(),
    );
    if partial {
        response_headers.insert(
            header::CONTENT_RANGE,
            format!("bytes {start}-{}/{size}", start + length - 1)
                .parse()
                .unwrap(),
        );
    }
    response
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
    fn download_ranges_unicode_names_empty_files_and_directory_archives() {
        with_test_home(|home| {
            let path = home.join("中文 空格.txt");
            std::fs::write(&path, b"0123456789").unwrap();
            crate::filedb::add_file("中文 空格.txt", path.to_str().unwrap(), None).unwrap();
            let dir = home.join("folder");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("hello.txt"), b"hello").unwrap();
            crate::filedb::add_file("folder", dir.to_str().unwrap(), None).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = super::super::build_app(home.to_path_buf());
                let url = "/api/download?filename=%E4%B8%AD%E6%96%87%20%E7%A9%BA%E6%A0%BC.txt";
                let response = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(url)
                            .header("range", "bytes=5-")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
                assert_eq!(response.headers()["content-range"], "bytes 5-9/10");
                assert_eq!(response.headers()["accept-ranges"], "bytes");
                assert!(response.headers()["content-disposition"]
                    .to_str()
                    .unwrap()
                    .contains("filename*=UTF-8''%E4%B8%AD%E6%96%87%20"));
                assert_eq!(
                    to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
                    b"56789"
                );
                let invalid = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(url)
                            .header("range", "bytes=100-")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(invalid.status(), StatusCode::RANGE_NOT_SATISFIABLE);
                let archive = app
                    .oneshot(
                        Request::builder()
                            .uri("/api/download?filename=folder&timestamp=1")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(archive.status(), StatusCode::OK);
                let archive = to_bytes(archive.into_body(), 1024 * 1024).await.unwrap();
                let mut archive = zip::ZipArchive::new(std::io::Cursor::new(archive)).unwrap();
                assert_eq!(archive.by_name("hello.txt").unwrap().size(), 5);
            });
        });
    }

    #[test]
    fn archive_cannot_include_denied_files_or_escaping_symlinks() {
        with_test_home(|home| {
            let dir = home.join("shared");
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(dir.join("visible.txt"), b"visible").unwrap();
            std::fs::write(dir.join("secret.txt"), b"secret").unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(home, dir.join("outside")).unwrap();
            let path = super::super::fileutil::temp_dir()
                .join(format!("test-{}.zip", uuid::Uuid::new_v4()));
            let permissions = json!({"denyAccess":dir.join("secret.txt").canonicalize().unwrap().to_string_lossy()});
            crate::ziputil::zip_directory(&dir, &path, &permissions).unwrap();
            let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
            assert!(archive.by_name("visible.txt").is_ok());
            assert!(archive.by_name("secret.txt").is_err());
            assert!(archive.by_name("outside/").is_err());
            let _ = std::fs::remove_file(&path);
        });
    }
}
