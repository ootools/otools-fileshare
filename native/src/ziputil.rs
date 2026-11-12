//! 流式写入临时 ZIP；逐条应用权限，不跟随目录符号链接。

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde_json::Value;
use tokio::sync::watch;
use zip::write::SimpleFileOptions;

use crate::account;

pub fn zip_directory(source: &Path, output: &Path, permissions: &Value) -> Result<(), String> {
    zip_directory_with_cancel(source, output, permissions, None)
}

pub fn zip_directory_with_cancel(
    source: &Path,
    output: &Path,
    permissions: &Value,
    stopping: Option<watch::Receiver<bool>>,
) -> Result<(), String> {
    fn visit(
        root: &Path,
        current: &Path,
        archive: &mut zip::ZipWriter<File>,
        permissions: &Value,
        stopping: &Option<watch::Receiver<bool>>,
    ) -> Result<(), String> {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(1))
            .large_file(true);
        for entry in std::fs::read_dir(current).map_err(|error| error.to_string())? {
            if stopping.as_ref().is_some_and(|stop| *stop.borrow()) {
                return Err("服务已停止".to_string());
            }
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if crate::server::fileutil::is_internal_name(&name) || name.contains(['\\', ':']) {
                continue;
            }
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_symlink()
            {
                continue;
            }
            let canonical = path.canonicalize().map_err(|error| error.to_string())?;
            if !canonical.starts_with(root)
                || !account::can_access_path(&canonical.to_string_lossy(), permissions)
            {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if canonical.is_dir() {
                archive
                    .add_directory(format!("{relative}/"), options)
                    .map_err(|error| error.to_string())?;
                visit(root, &canonical, archive, permissions, stopping)?;
            } else if canonical.is_file() {
                archive
                    .start_file(relative, options)
                    .map_err(|error| error.to_string())?;
                let mut file = File::open(canonical).map_err(|error| error.to_string())?;
                let mut buffer = [0; 64 * 1024];
                loop {
                    if stopping.as_ref().is_some_and(|stop| *stop.borrow()) {
                        return Err("服务已停止".to_string());
                    }
                    let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
                    if count == 0 {
                        break;
                    }
                    archive
                        .write_all(&buffer[..count])
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        Ok(())
    }
    let root = source.canonicalize().map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err("不是文件夹".to_string());
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut archive = zip::ZipWriter::new(File::create(output).map_err(|error| error.to_string())?);
    visit(&root, &root, &mut archive, permissions, &stopping)?;
    archive.finish().map_err(|error| error.to_string())?;
    Ok(())
}
