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
    fn anonymous_login_cannot_mint_credentials_that_bypass_enabled_authentication() {
        with_test_home(|home| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = crate::server::build_app(home.to_path_buf());
                let login = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/login")
                            .method("POST")
                            .header("content-type", "application/json")
                            .body(Body::from("{}"))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let body: Value =
                    serde_json::from_slice(&to_bytes(login.into_body(), 10000).await.unwrap())
                        .unwrap();
                let token = body["data"]["Authorization"].as_str().unwrap_or_default();
                crate::setting::update_auth_enable(&json!(true)).unwrap();
                let files = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/files")
                            .header("authorization", token)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let body: Value =
                    serde_json::from_slice(&to_bytes(files.into_body(), 10000).await.unwrap())
                        .unwrap();
                assert_eq!(body["code"], 401);
                let direct = app
                    .oneshot(
                        Request::builder()
                            .uri(format!("/api/download?filename=anything&token={token}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(direct.status(), StatusCode::FORBIDDEN);
            });
        });
    }

    #[test]
    fn internal_upload_staging_is_not_listed_resolved_or_archived() {
        with_test_home(|home| {
            let dir = home.join("shared");
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(dir.join("visible.txt"), "visible").unwrap();
            std::fs::write(dir.join(".otools-fileshare-secret.part"), "private").unwrap();
            let staging = dir.join(".otools-fileshare-uploads");
            std::fs::create_dir(&staging).unwrap();
            std::fs::write(staging.join("session.part"), "private").unwrap();
            crate::filedb::add_file("shared", dir.to_str().unwrap(), None).unwrap();
            assert!(crate::server::fileutil::parse_path(
                "shared/.otools-fileshare-uploads/session.part"
            )
            .is_err());
            assert!(
                crate::server::fileutil::parse_path("shared/.otools-fileshare-secret.part")
                    .is_err()
            );
            let listed = crate::server::fileutil::list_dir(&dir).unwrap();
            assert_eq!(listed.len(), 1);
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(
                    dir.join(".otools-fileshare-secret.part"),
                    dir.join("alias.txt"),
                )
                .unwrap();
                assert!(crate::server::fileutil::parse_path("shared/alias.txt").is_err());
            }
            let output = home.join("archive.zip");
            crate::ziputil::zip_directory(&dir, &output, &json!({})).unwrap();
            let mut archive = zip::ZipArchive::new(std::fs::File::open(&output).unwrap()).unwrap();
            assert!(archive.by_name("visible.txt").is_ok());
            assert!(archive.by_name(".otools-fileshare-secret.part").is_err());
            assert!(archive
                .by_name(".otools-fileshare-uploads/session.part")
                .is_err());
        });
    }

    #[cfg(unix)]
    #[test]
    fn replaced_share_root_and_unsafe_archive_names_are_rejected() {
        with_test_home(|home| {
            let dir = home.join("root");
            let outside = home.join("outside");
            std::fs::create_dir(&dir).unwrap();
            std::fs::create_dir(&outside).unwrap();
            std::fs::write(outside.join("secret.txt"), "secret").unwrap();
            crate::filedb::add_file("root", dir.to_str().unwrap(), None).unwrap();
            std::fs::remove_dir(&dir).unwrap();
            std::os::unix::fs::symlink(&outside, &dir).unwrap();
            assert!(crate::server::fileutil::parse_path("root/secret.txt").is_err());
            std::fs::write(outside.join("..\\escape.txt"), "unsafe").unwrap();
            let archive = home.join("safe.zip");
            crate::ziputil::zip_directory(&outside, &archive, &json!({})).unwrap();
            let mut archive = zip::ZipArchive::new(std::fs::File::open(archive).unwrap()).unwrap();
            for index in 0..archive.len() {
                let entry = archive.by_index(index).unwrap();
                assert!(!entry.name().split('/').any(|part| part == ".."));
                assert!(!entry.name().contains('\\'));
            }
        });
    }

    #[test]
    fn native_validation_errors_survive_the_hosts_string_error_contract() {
        let result = crate::dispatch("updateUploadPath", json!(""), "");
        assert_eq!(result["error"].as_str(), Some("更新上传路径失败，路径为空"));
    }

    #[test]
    fn plugin_shutdown_closes_admission_to_later_start_calls() {
        with_test_home(|home| {
            let page = home.join("plugin/web-static/page_web");
            std::fs::create_dir_all(&page).unwrap();
            std::fs::write(page.join("index.html"), "<html>test</html>").unwrap();
            crate::config::set_plugin_dir(home.join("plugin").to_str().unwrap());
            let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = probe.local_addr().unwrap().port();
            drop(probe);
            crate::setting::update_port(&json!(port)).unwrap();
            assert_eq!(crate::dispatch("startServer", json!(null), "")["ok"], true);
            assert_eq!(
                crate::dispatch("__otools_shutdown__", json!(null), "")["ok"],
                true
            );
            let later = crate::dispatch("startServer", json!(null), "");
            assert_eq!(later["ok"], false);
            crate::server::stop().unwrap();
            crate::runtime::shutdown();
        });
    }

    #[test]
    fn delayed_text_upload_is_cancelled_when_service_stops() {
        with_test_home(|home| {
            crate::store::set_item("allowAnonymousUpload", json!(true)).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (stop_tx,stopping)=tokio::sync::watch::channel(false);
                let state=std::sync::Arc::new(crate::server::AppState{static_root:home.to_path_buf(),stopping,stop_tx:stop_tx.clone()});
                let app=crate::server::build_with_state(state);
                let (started_tx,started_rx)=tokio::sync::oneshot::channel();
                let (release_tx,release_rx)=tokio::sync::oneshot::channel();
                let body=async_stream::stream! {
                    yield Ok::<_,std::io::Error>(axum::body::Bytes::from_static(b"{\"message\":\""));
                    let _=started_tx.send(());
                    let _=release_rx.await;
                    yield Ok(axum::body::Bytes::from_static(b"late write\"}"));
                };
                let pending=tokio::spawn(async move{app.oneshot(Request::builder().method("POST").uri("/api/addText").header("content-type","application/json").body(Body::from_stream(body)).unwrap()).await.unwrap()});
                started_rx.await.unwrap();
                stop_tx.send(true).unwrap();
                let _=release_tx.send(());
                let response=pending.await.unwrap();
                assert_eq!(response.status(),StatusCode::SERVICE_UNAVAILABLE);
                assert!(crate::filedb::list_files().unwrap().is_empty());
            });
        });
    }

    #[test]
    fn tus_interrupted_finalization_recovers_without_duplicate_publication() {
        with_test_home(|home| {
            let upload = home.join("uploads");
            std::fs::create_dir(&upload).unwrap();
            crate::setting::update_upload_path(&json!(upload)).unwrap();
            crate::store::set_item("allowAnonymousUpload", json!(true)).unwrap();
            crate::setting::update_tus_enable(&json!(true)).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = crate::server::build_app(home.to_path_buf());
                let created = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/uploads")
                            .method("POST")
                            .header("tus-resumable", "1.0.0")
                            .header("upload-length", "4")
                            .header("upload-metadata", "filename dGVzdC50eHQ=")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let url = created.headers()["location"].to_str().unwrap();
                let id = url.rsplit('/').next().unwrap();
                let staging = upload.join(".otools-fileshare-uploads");
                let part = staging.join(format!("{id}.part"));
                let metadata = staging.join(format!("{id}.json"));
                std::fs::write(&part, "test").unwrap();
                let target = upload.join("test.txt");
                std::fs::hard_link(&part, &target).unwrap();
                let mut session: Value =
                    serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
                session["destination"] = json!(target.canonicalize().unwrap());
                std::fs::write(&metadata, session.to_string()).unwrap();
                let response = app
                    .oneshot(
                        Request::builder()
                            .uri(url)
                            .method("HEAD")
                            .header("tus-resumable", "1.0.0")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(crate::filedb::list_files().unwrap().len(), 1);
                assert!(!upload.join("test (1).txt").exists());
                let session: Value =
                    serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
                assert!(session["completed"].is_string());
            });
        });
    }

    #[cfg(unix)]
    #[test]
    fn resumable_session_symlink_is_rejected_before_writing() {
        with_test_home(|home| {
            let upload = home.join("uploads");
            std::fs::create_dir(&upload).unwrap();
            crate::setting::update_upload_path(&json!(upload)).unwrap();
            crate::store::set_item("allowAnonymousUpload", json!(true)).unwrap();
            crate::setting::update_tus_enable(&json!(true)).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let app = crate::server::build_app(home.to_path_buf());
                let created = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri("/api/uploads")
                            .method("POST")
                            .header("tus-resumable", "1.0.0")
                            .header("upload-length", "10")
                            .header("upload-metadata", "filename dGVzdC50eHQ=")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(created.status(), StatusCode::CREATED);
                let url = created.headers()["location"].to_str().unwrap();
                let id = url.rsplit('/').next().unwrap();
                let part = upload
                    .join(".otools-fileshare-uploads")
                    .join(format!("{id}.part"));
                let outside = home.join("outside");
                std::fs::write(&outside, b"01234").unwrap();
                std::fs::remove_file(&part).unwrap();
                std::os::unix::fs::symlink(&outside, &part).unwrap();
                let response = app
                    .oneshot(
                        Request::builder()
                            .uri(url)
                            .method("PATCH")
                            .header("tus-resumable", "1.0.0")
                            .header("upload-offset", "5")
                            .header("content-type", "application/offset+octet-stream")
                            .body(Body::from("56789"))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_ne!(response.status(), StatusCode::NO_CONTENT);
                assert_eq!(std::fs::read(&outside).unwrap(), b"01234");
            });
        });
    }
}
