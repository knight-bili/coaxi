//! 改名不能把已有实例误判成首次运行；这里仅检查，不改写旧数据。
use std::{io, path::{Path, PathBuf}};

pub fn legacy_authority(root: &Path) -> io::Result<Option<PathBuf>> {
    for name in ["rovai.sqlite", "rovai.sqlite-wal", "rovai.sqlite-shm", "rovai.sqlite-journal", ".rovai-authority-migration-v1.json"] {
        let path = root.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Some(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {},
            Err(error) => return Err(error),
        }
    }
    Ok(None)
}

pub fn reject_legacy_authority(root: &Path) -> anyhow::Result<()> {
    if let Some(path) = legacy_authority(root)? {
        anyhow::bail!("brand_migration_required: 检测到旧版 Coaxi 数据，尚未完成整份实例迁移。未创建空数据库、未改写原件：{}", path.display());
    }
    Ok(())
}

pub fn reject_legacy_desktop_root(root: &Path) -> anyhow::Result<()> {
    reject_legacy_authority(&root.join("Core"))?;
    // 只检查已发布版本使用过的同级默认目录，不搜索用户的其他文件夹。
    if root.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Coaxi")) {
        if let Some(parent) = root.parent() {
            let legacy = parent.join("Codeg Rovai Workbench");
            if legacy.try_exists()? {
                anyhow::bail!("brand_migration_required: 发现旧版 Coaxi 实例。请先完成整份数据迁移；未创建新的空实例、未改写原件：{}", legacy.display());
            }
        }
    }
    Ok(())
}

pub fn reject_legacy_server_default(home: &Path) -> anyhow::Result<()> {
    for relative in [".rovai-server", ".rovai/server", ".coaxi/server"] {
        let root = home.join(relative);
        for name in ["rovai.sqlite", "rovai.sqlite-wal", "rovai.sqlite-journal", "rovai.sqlite-shm", "coaxi.sqlite", "server-layout.json"] {
            let path = root.join(name);
            match std::fs::symlink_metadata(&path) {
                Ok(_) => anyhow::bail!("brand_migration_required: 发现旧版 Coaxi 服务端实例。未创建空实例、未改写原件：{}", root.display()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_main_sidecars_and_manifest_never_look_like_an_empty_instance() {
        let parent = std::env::temp_dir().join(format!("coaxi-brand-guard-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&parent).unwrap();
        for name in ["rovai.sqlite", "rovai.sqlite-wal", "rovai.sqlite-shm", "rovai.sqlite-journal", ".rovai-authority-migration-v1.json"] {
            let path = parent.join(name);
            std::fs::write(&path, b"original authority bytes").unwrap();
            assert!(reject_legacy_authority(&parent).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"original authority bytes");
            assert!(!parent.join("coaxi.sqlite").exists());
            std::fs::remove_file(path).unwrap();
        }
        assert!(reject_legacy_authority(&parent).is_ok());
        std::fs::remove_dir(&parent).unwrap();
    }
    #[test]
    fn renamed_defaults_do_not_bypass_previous_desktop_or_server_instances() {
        let parent = std::env::temp_dir().join(format!("coaxi-default-guard-{}", uuid::Uuid::new_v4()));
        let old = parent.join("Codeg Rovai Workbench");
        std::fs::create_dir_all(&old).unwrap();
        assert!(reject_legacy_desktop_root(&parent.join("Coaxi")).is_err());
        assert!(!parent.join("Coaxi").exists());
        std::fs::remove_dir(&old).unwrap();
        for relative in [".rovai-server", ".rovai/server", ".coaxi/server"] {
            let root = parent.join(relative);
            std::fs::create_dir_all(&root).unwrap();
            let path = root.join("rovai.sqlite-wal");
            std::fs::write(&path, b"pending authority").unwrap();
            assert!(reject_legacy_server_default(&parent).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"pending authority");
            assert!(!parent.join(".coaxi-server").exists());
            std::fs::remove_file(path).unwrap();
        }
        // 这里只清理本测试创建并核对过的目录。
        std::fs::remove_dir_all(&parent).unwrap();
    }
}
