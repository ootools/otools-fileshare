//! 本地浏览器验收用服务（仅 loopback），不写真实用户配置。
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let plugin_dir = std::env::args().nth(1).expect("usage: serve PLUGIN_DIR");
    let root = PathBuf::from(&plugin_dir);
    let upload = dirs::home_dir().expect("HOME").join("Downloads");
    std::fs::create_dir_all(&upload).unwrap();
    otools_fileshare_native::config::set_plugin_dir(&plugin_dir);
    otools_fileshare_native::setting::update_upload_path(&serde_json::json!(upload)).unwrap();
    otools_fileshare_native::store::set_item("allowAnonymousUpload", serde_json::json!(true))
        .unwrap();
    otools_fileshare_native::setting::update_tus_enable(&serde_json::json!(true)).unwrap();
    let sample = dirs::home_dir().unwrap().join("浏览器验收.txt");
    std::fs::write(&sample, "OTools file-share real Rust backend\n").unwrap();
    otools_fileshare_native::filedb::add_file("浏览器验收.txt", sample.to_str().unwrap(), None)
        .unwrap();
    otools_fileshare_native::filedb::add_text("你好，文件共享。", Some("local-test")).unwrap();
    let result = otools_fileshare_native::server::start_on(
        "127.0.0.1:6548".parse().unwrap(),
        root.join("web-static/page_web"),
    )
    .unwrap();
    println!("{result}");
    std::io::stdout().flush().unwrap();
    loop {
        std::thread::park();
    }
}
