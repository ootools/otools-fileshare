//! 插件自管的局域网 HTTP 服务及启停协商。

pub mod fileutil;
pub mod routes_download;
pub mod routes_files;
pub mod routes_misc;
pub mod routes_upload;
pub mod sse;
pub mod tus;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tokio::sync::watch;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

use crate::{config, event_sink, jwt, runtime, setting};

#[derive(Clone)]
pub struct Permissions(pub Value);

#[derive(Clone)]
pub struct AppState {
    pub static_root: PathBuf,
    pub stopping: watch::Receiver<bool>,
    pub stop_tx: watch::Sender<bool>,
}

struct Running {
    addr: SocketAddr,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

static SERVICE: Mutex<Option<Running>> = Mutex::new(None);

pub fn api_error(code: u16, message: impl Into<String>) -> Response {
    Json(json!({"code":code,"message":message.into()})).into_response()
}

pub fn auth_enabled() -> Result<bool, String> {
    Ok(setting::get_auth_enable()?.as_bool().unwrap_or(false))
}

async fn auth(mut request: Request, next: Next) -> Response {
    if !request.uri().path().starts_with("/api/") {
        return next.run(request).await;
    }
    match auth_enabled() {
        Ok(false) => {
            request
                .extensions_mut()
                .insert(Permissions(jwt::admin_permissions()));
            return next.run(request).await;
        }
        Err(error) => return api_error(500, error),
        Ok(true) => {}
    }
    let path = request.uri().path();
    if path == "/api/login" || path == "/api/registrySSE" || path == "/api/download" {
        return next.run(request).await;
    }
    let token = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let permissions =
        jwt::verify(token).and_then(|payload| jwt::resolve_permissions(&payload).ok().flatten());
    if let Some(permissions) = permissions {
        request.extensions_mut().insert(Permissions(permissions));
        next.run(request).await
    } else {
        api_error(401, "认证失败")
    }
}

async fn static_page(State(state): State<Arc<AppState>>, request: Request) -> Response {
    if request.uri().path().starts_with("/api/") {
        return (StatusCode::NOT_FOUND, "API not found").into_response();
    }
    let index = state.static_root.join("index.html");
    let service = ServeDir::new(&state.static_root).fallback(ServeFile::new(index));
    match service.oneshot(request).await {
        Ok(response) => response.map(Body::new),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub fn build_app(static_root: PathBuf) -> Router {
    let (stop_tx, stopping) = watch::channel(false);
    build_with_state(Arc::new(AppState {
        static_root,
        stopping,
        stop_tx,
    }))
}

pub(crate) fn build_with_state(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/login", post(routes_misc::login))
        .route("/api/files", axum::routing::get(routes_files::list_files))
        .route(
            "/api/getSetting",
            axum::routing::get(routes_misc::get_setting),
        )
        .route(
            "/api/tusConfig",
            axum::routing::get(routes_misc::tus_config),
        )
        .route("/api/logout", axum::routing::get(routes_misc::logout))
        .route(
            "/api/download",
            axum::routing::get(routes_download::download),
        )
        .route(
            "/api/addFile",
            post(routes_upload::add_file).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route("/api/addText", post(routes_upload::add_text))
        .route("/api/uploads", post(tus::create))
        .route(
            "/api/uploads/{id}",
            axum::routing::head(tus::head)
                .patch(tus::patch)
                .layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route("/api/registrySSE", axum::routing::get(sse::registry))
        .fallback(static_page)
        .with_state(state)
        .layer(middleware::from_fn(auth))
}

pub fn start_on(addr: SocketAddr, static_root: PathBuf) -> Result<Value, String> {
    let mut service = SERVICE.lock().map_err(|_| "服务状态锁错误")?;
    if let Some(running) = service.as_ref() {
        return Ok(
            json!({"success":true,"message":"服务已启动","listenAddress":running.addr.to_string(),"url":setting::get_url()?}),
        );
    }
    if !static_root.join("index.html").is_file() {
        return Err("手机端页面未构建，请先运行 build-web".to_string());
    }
    let listener =
        std::net::TcpListener::bind(addr).map_err(|error| format!("服务启动失败: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let addr = listener.local_addr().map_err(|error| error.to_string())?;
    let (stop, stopping) = watch::channel(false);
    let state = Arc::new(AppState {
        static_root,
        stop_tx: stop.clone(),
        stopping: stopping.clone(),
    });
    let task = runtime::with_rt(|| {
        let listener =
            tokio::net::TcpListener::from_std(listener).map_err(|error| error.to_string())?;
        Ok::<_, String>(tokio::spawn(async move {
            let mut stopping = stopping;
            axum::serve(
                listener,
                build_with_state(state).into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async move {
                let _ = stopping.changed().await;
            })
            .await
        }))
    })?;
    *service = Some(Running { addr, stop, task });
    event_sink::push("fileshare:server.statusChange", json!({"status":"start"}));
    Ok(
        json!({"success":true,"message":"服务启动成功","listenAddress":addr.to_string(),"url":setting::get_url()?}),
    )
}

pub fn start() -> Result<Value, String> {
    let port = setting::get_port()?;
    let port = port
        .as_u64()
        .or_else(|| port.as_str().and_then(|value| value.parse().ok()))
        .filter(|port| *port > 0 && *port <= 65535)
        .ok_or("端口必须在 1–65535 之间")?;
    let ip = if crate::iputil::get_ip_family()? == "ipv6" {
        "::"
    } else {
        "0.0.0.0"
    };
    let plugin_dir = config::plugin_dir();
    let root = if plugin_dir.is_empty() {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
    } else {
        PathBuf::from(plugin_dir)
    };
    start_on(
        format!("[{ip}]:{port}")
            .parse()
            .or_else(|_| format!("{ip}:{port}").parse())
            .map_err(|error: std::net::AddrParseError| error.to_string())?,
        root.join("web-static/page_web"),
    )
}

pub fn stop() -> Result<Value, String> {
    let mut service = SERVICE.lock().map_err(|_| "服务状态锁错误")?;
    if let Some(mut running) = service.take() {
        let _ = running.stop.send(true);
        runtime::with_rt(|| {
            tokio::runtime::Handle::current().block_on(async {
                if tokio::time::timeout(Duration::from_secs(1), &mut running.task)
                    .await
                    .is_err()
                {
                    running.task.abort();
                    let _ = running.task.await;
                }
            })
        });
        // 先广播停止，让所有 handler 取消，再销毁整个插件 runtime；
        // 仅 abort axum serve 任务不会结束独立 connection task。
        runtime::shutdown();
        event_sink::push("fileshare:server.statusChange", json!({"status":"stop"}));
    }
    Ok(json!({"success":true,"message":"服务已停止"}))
}

pub fn status() -> &'static str {
    if SERVICE.lock().is_ok_and(|service| {
        service
            .as_ref()
            .is_some_and(|running| !running.task.is_finished())
    }) {
        "start"
    } else {
        "stop"
    }
}

#[cfg(test)]
mod tests {
    use crate::store::test_support::with_test_home;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use std::path::Path;
    use tower::ServiceExt;

    fn static_page(home: &Path) -> std::path::PathBuf {
        let root = home.join("receiver");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<html>receiver</html>").unwrap();
        root
    }

    #[test]
    fn auth_whitelist_and_spa_fallback_are_separate_from_unknown_api_routes() {
        with_test_home(|home| {
            let root = static_page(home);
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = super::build_app(root);
                let spa = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/folder/deep/link")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(spa.status(), StatusCode::OK);
                assert_eq!(
                    to_bytes(spa.into_body(), 1024).await.unwrap().as_ref(),
                    b"<html>receiver</html>"
                );
                let unknown = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/unknown")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
                crate::setting::update_auth_enable(&json!(true)).unwrap();
                let unauthorized = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/files")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let value: Value = serde_json::from_slice(
                    &to_bytes(unauthorized.into_body(), 2048).await.unwrap(),
                )
                .unwrap();
                assert_eq!(value["code"], 401);
                let login = app
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri("/api/login")
                            .header("content-type", "application/json")
                            .body(Body::from(r#"{"username":"test","password":"x"}"#))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let value: Value =
                    serde_json::from_slice(&to_bytes(login.into_body(), 2048).await.unwrap())
                        .unwrap();
                assert_ne!(value["code"], 401);
            });
        });
    }

    #[test]
    fn lifecycle_releases_port_and_failed_bind_does_not_claim_started() {
        with_test_home(|home| {
            let root = static_page(home);
            super::stop().unwrap();
            let result = super::start_on("127.0.0.1:0".parse().unwrap(), root.clone()).unwrap();
            let addr: std::net::SocketAddr =
                result["listenAddress"].as_str().unwrap().parse().unwrap();
            assert_eq!(super::status(), "start");
            assert!(std::net::TcpStream::connect(addr).is_ok());
            super::stop().unwrap();
            assert_eq!(super::status(), "stop");
            let occupied = std::net::TcpListener::bind(addr).unwrap();
            assert!(super::start_on(addr, root).is_err());
            assert_eq!(super::status(), "stop");
            drop(occupied);
            crate::runtime::shutdown();
        });
    }
}
