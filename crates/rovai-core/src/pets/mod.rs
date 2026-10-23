//! Source-compatible sprite packages. Pet assets own no execution or model state.
pub mod marketplace;
pub mod model;
pub use validation::{validate_pet_id, validate_spritesheet};
pub mod validation;
use crate::platform::private_storage::{
    atomic_write_private_bytes, atomic_write_private_json, prepare_private_directory,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path};
fn safe(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(!metadata.file_type().is_symlink(), "pets.linked_path");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(metadata.file_attributes() & 0x400 == 0, "pets.linked_path");
    }
    Ok(())
}
pub fn read_manifest(root: &Path, id: &str) -> Result<model::PetManifest> {
    validation::validate_pet_id(id)?;
    let directory = root.join(id);
    safe(&directory)?;
    let path = directory.join(model::PET_MANIFEST_FILENAME);
    safe(&path)?;
    ensure!(
        fs::metadata(&path)?.len() <= 64 * 1024,
        "pets.manifest_limit"
    );
    let manifest: model::PetManifest = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        manifest.id == id
            && !manifest.display_name.trim().is_empty()
            && manifest.display_name.len() <= 256
            && manifest
                .description
                .as_ref()
                .is_none_or(|value| value.len() <= 16000),
        "pets.invalid_manifest"
    );
    Ok(manifest)
}
pub fn list(root: &Path) -> Result<Vec<Value>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    safe(root)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if validation::validate_pet_id(&id).is_err() || !entry.file_type()?.is_dir() {
            continue;
        }
        match read_manifest(root, &id) {
            Ok(manifest) => entries.push(serde_json::to_value(manifest)?),
            Err(error) => entries.push(
                json!({"id":id,"displayName":id,"unavailable":true,"message":error.to_string()}),
            ),
        };
        ensure!(entries.len() <= 1000, "pets.catalog_limit");
    }
    entries.sort_by(|a, b| a["displayName"].as_str().cmp(&b["displayName"].as_str()));
    Ok(entries)
}
pub fn asset(root: &Path, id: &str) -> Result<(Vec<u8>, String)> {
    read_manifest(root, id)?;
    let path = root.join(id).join(model::SPRITESHEET_FILENAME);
    safe(&path)?;
    ensure!(
        fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
        "pets.sprite_limit"
    );
    let bytes = fs::read(path)?;
    let format = validation::validate_spritesheet(&bytes)?;
    Ok((bytes, format.mime().into()))
}
pub fn install(root: &Path, mut manifest: model::PetManifest, bytes: &[u8]) -> Result<Value> {
    validation::validate_pet_id(&manifest.id)?;
    validation::validate_spritesheet(bytes)?;
    ensure!(
        serde_json::to_vec(&manifest)?.len() <= 64 * 1024
            && !manifest.display_name.trim().is_empty()
            && manifest.display_name.len() <= 256,
        "pets.invalid_manifest"
    );
    manifest.spritesheet_path = model::SPRITESHEET_FILENAME.into();
    prepare_private_directory(root)?;
    let destination = root.join(&manifest.id);
    if destination.exists() {
        let old = read_manifest(root, &manifest.id)?;
        let (old_bytes, _) = asset(root, &manifest.id)?;
        ensure!(
            serde_json::to_value(old)? == serde_json::to_value(&manifest)? && old_bytes == bytes,
            "pets.id_already_installed"
        );
        return Ok(json!({"id":manifest.id,"duplicate":true}));
    }
    let temporary = root.join(format!(".install-{}", uuid::Uuid::new_v4()));
    prepare_private_directory(&temporary)?;
    let result = (|| -> Result<Value> {
        atomic_write_private_json(&temporary.join(model::PET_MANIFEST_FILENAME), &manifest)?;
        atomic_write_private_bytes(&temporary.join(model::SPRITESHEET_FILENAME), bytes)?;
        ensure!(!destination.exists(), "pets.id_already_installed");
        fs::rename(&temporary, &destination)?;
        Ok(json!({"id":manifest.id,"duplicate":false}))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}
pub fn import_directory(root: &Path, source: &Path) -> Result<Value> {
    safe(source)?;
    ensure!(source.is_dir(), "pets.directory_required");
    let manifest_path = source.join(model::PET_MANIFEST_FILENAME);
    safe(&manifest_path)?;
    ensure!(
        fs::metadata(&manifest_path)?.len() <= 64 * 1024,
        "pets.manifest_limit"
    );
    let manifest: model::PetManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    let sprite = source.join(model::SPRITESHEET_FILENAME);
    safe(&sprite)?;
    ensure!(
        fs::metadata(&sprite)?.len() <= 16 * 1024 * 1024,
        "pets.sprite_limit"
    );
    install(root, manifest, &fs::read(sprite)?)
}
pub fn package(root: &Path, bytes: &[u8]) -> Result<Value> {
    ensure!(bytes.len() <= 32 * 1024 * 1024, "pets.package_limit");
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut manifest = None;
    let mut sprite = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry
            .enclosed_name()
            .context("pets.invalid_package_path")?
            .to_string_lossy()
            .replace('\\', "/");
        ensure!(
            entry
                .unix_mode()
                .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "pets.linked_package_entry"
        );
        match name.as_str() {
            "pet.json" => {
                ensure!(
                    manifest.is_none() && entry.size() <= 64 * 1024,
                    "pets.invalid_manifest_entry"
                );
                let mut bytes = Vec::new();
                (&mut entry).take(64 * 1024 + 1).read_to_end(&mut bytes)?;
                manifest = Some(serde_json::from_slice::<model::PetManifest>(&bytes)?);
            }
            "spritesheet.webp" | "spritesheet.png" => {
                ensure!(
                    sprite.is_none() && entry.size() <= 16 * 1024 * 1024,
                    "pets.invalid_sprite_entry"
                );
                let mut bytes = Vec::new();
                (&mut entry)
                    .take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                sprite = Some(bytes)
            }
            _ => {}
        }
    }
    install(
        root,
        manifest.context("pets.manifest_missing")?,
        &sprite.context("pets.sprite_missing")?,
    )
}

#[derive(Debug)]
pub struct MarketError {
    message: String,
}
impl std::fmt::Display for MarketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for MarketError {}
impl From<anyhow::Error> for MarketError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}
impl MarketError {
    fn io(error: impl std::fmt::Display) -> Self {
        Self {
            message: format!("pets.io: {error}"),
        }
    }
    fn network(message: impl AsRef<str>) -> Self {
        Self {
            message: format!("pets.marketplace_network: {}", message.as_ref()),
        }
    }
    fn invalid_input(message: impl AsRef<str>) -> Self {
        Self {
            message: format!("pets.invalid_package: {}", message.as_ref()),
        }
    }
    fn already_exists(message: impl AsRef<str>) -> Self {
        Self {
            message: format!("pets.already_installed: {}", message.as_ref()),
        }
    }
    fn task_execution_failed(message: impl AsRef<str>) -> Self {
        Self {
            message: format!("pets.install_failed: {}", message.as_ref()),
        }
    }
}
pub fn ensure_pets_root_or_create(root: &Path) -> Result<std::path::PathBuf> {
    prepare_private_directory(root)
}
pub fn list_existing_ids(root: &Path) -> Result<std::collections::HashSet<String>> {
    Ok(list(root)?
        .into_iter()
        .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
        .collect())
}
