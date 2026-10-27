//! Fixed Coaxi driver artifacts, stored outside agent executable directories.
use crate::computer::driver::{artifact_for_current_platform, DRIVER_COMMAND, DRIVER_VERSION};
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};
static FILES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
fn cache() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("computer.no_data_directory")?
        .join("CoaxiAI")
        .join("computer-driver"))
}
fn executable() -> String {
    if cfg!(windows) {
        format!("{DRIVER_COMMAND}.exe")
    } else {
        DRIVER_COMMAND.into()
    }
}
pub fn cached_driver_path() -> Option<PathBuf> {
    let path = cache().ok()?.join(DRIVER_VERSION).join(executable());
    path.is_file().then_some(path)
}
pub fn installed_driver_versions() -> Result<Vec<String>> {
    let root = cache()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut versions = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join(executable()).is_file() {
            versions.push(entry.file_name().to_string_lossy().into_owned())
        }
    }
    versions.sort();
    versions.reverse();
    Ok(versions)
}
pub async fn forget_cached_driver() -> Result<()> {
    let _lock = FILES.lock().await;
    let root = cache()?;
    if root.exists() {
        let canonical = fs::canonicalize(&root)?;
        ensure!(
            canonical.file_name().and_then(|p| p.to_str()) == Some("computer-driver")
                && canonical
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|p| p.to_str())
                    == Some("CoaxiAI"),
            "computer.invalid_cache"
        );
        ensure!(
            !fs::symlink_metadata(&root)?.file_type().is_symlink(),
            "computer.linked_cache"
        );
        fs::remove_dir_all(&root)?;
    }
    Ok(())
}
pub async fn ensure_driver(on_progress: impl Fn(&str)) -> Result<PathBuf> {
    let _lock = FILES.lock().await;
    let artifact = artifact_for_current_platform().context("computer.platform_not_supported")?;
    if let Some(path) = cached_driver_path() {
        let actual = format!("{:x}", Sha256::digest(fs::read(&path)?));
        ensure!(
            actual == artifact.executable_sha256,
            "computer.cached_driver_digest_changed"
        );
        return Ok(path);
    }
    on_progress("正在下载固定版本的电脑操作驱动");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let mut response = client.get(artifact.url).send().await?.error_for_status()?;
    ensure!(
        response.url().scheme() == "https",
        "computer.insecure_download"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 256 * 1024 * 1024,
            "computer.download_limit"
        );
        bytes.extend_from_slice(&chunk)
    }
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == artifact.archive_sha256,
        "computer.archive_digest_changed"
    );
    on_progress("正在核对并安装驱动");
    let name = executable();
    let mut executable_bytes = None;
    if artifact.url.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        for index in 0..archive.len() {
            let mut file = archive.by_index(index)?;
            if file.is_dir() {
                continue;
            }
            let path = file
                .enclosed_name()
                .context("computer.invalid_archive_entry")?;
            if path.file_name().and_then(|part| part.to_str()) != Some(name.as_str()) {
                continue;
            }
            ensure!(
                executable_bytes.is_none()
                    && file.size() <= 256 * 1024 * 1024
                    && file
                        .unix_mode()
                        .is_none_or(|mode| mode & 0o170000 != 0o120000),
                "computer.invalid_executable_entry"
            );
            let mut output = Vec::new();
            (&mut file)
                .take(256 * 1024 * 1024 + 1)
                .read_to_end(&mut output)?;
            ensure!(
                output.len() <= 256 * 1024 * 1024,
                "computer.executable_limit"
            );
            executable_bytes = Some(output)
        }
    } else {
        let mut archive =
            tar::Archive::new(flate2::read::GzDecoder::new(std::io::Cursor::new(bytes)));
        for entry in archive.entries()? {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            if entry.path()?.file_name().and_then(|part| part.to_str()) != Some(name.as_str()) {
                continue;
            }
            ensure!(
                executable_bytes.is_none() && entry.size() <= 256 * 1024 * 1024,
                "computer.invalid_executable_entry"
            );
            let mut output = Vec::new();
            (&mut entry)
                .take(256 * 1024 * 1024 + 1)
                .read_to_end(&mut output)?;
            ensure!(
                output.len() <= 256 * 1024 * 1024,
                "computer.executable_limit"
            );
            executable_bytes = Some(output)
        }
    }
    let bytes = executable_bytes.context("computer.executable_missing")?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes)) == artifact.executable_sha256,
        "computer.executable_digest_changed"
    );
    let directory = cache()?.join(DRIVER_VERSION);
    fs::create_dir_all(&directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    }
    let destination = directory.join(&name);
    let temporary = directory.join(format!(".{}", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))?;
        }
        fs::hard_link(&temporary, &destination).context("computer.cache_changed")?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result?;
    Ok(destination)
}
