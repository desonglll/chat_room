use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Component, Path},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(super) const FORMAT_VERSION: u32 = 2;
pub(super) const DUMP_FILE: &str = "database.dump";
pub(super) const ATTACHMENTS_DIR: &str = "attachments";
pub(super) const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format_version: u32,
    pub created_at: DateTime<Utc>,
    pub database_kind: String,
    pub dump_file: String,
    pub attachments_directory: String,
    #[serde(default = "legacy_backup_includes_files")]
    pub includes_files: bool,
    pub files: Vec<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

const fn legacy_backup_includes_files() -> bool {
    true
}

pub fn read_and_verify(root: &Path) -> Result<BackupManifest> {
    let bytes = fs::read(root.join(MANIFEST_FILE))
        .with_context(|| format!("read backup manifest from {}", root.display()))?;
    let manifest: BackupManifest =
        serde_json::from_slice(&bytes).context("decode backup manifest")?;
    if !(1..=FORMAT_VERSION).contains(&manifest.format_version)
        || manifest.database_kind != "postgres"
    {
        bail!("unsupported backup format or database kind");
    }
    if manifest.dump_file != DUMP_FILE || manifest.attachments_directory != ATTACHMENTS_DIR {
        bail!("backup manifest uses unsupported paths");
    }

    let mut expected = BTreeMap::new();
    for record in &manifest.files {
        if expected.insert(record.path.clone(), record).is_some() {
            bail!("duplicate file in backup manifest: {}", record.path);
        }
        let path = safe_join(root, &record.path)?;
        let actual = file_record(&path, Path::new(&record.path))?;
        if actual.size_bytes != record.size_bytes || actual.sha256 != record.sha256 {
            bail!("backup checksum mismatch: {}", record.path);
        }
    }
    if !expected.contains_key(DUMP_FILE) {
        bail!("backup manifest does not include {DUMP_FILE}");
    }
    let attachment_prefix = format!("{ATTACHMENTS_DIR}/");
    let has_attachment_files = expected
        .keys()
        .any(|path| path.starts_with(&attachment_prefix));
    if has_attachment_files && !manifest.includes_files {
        bail!("backup file scope does not match the manifest");
    }

    let mut actual = Vec::new();
    collect_relative_files(root, root, &mut actual)?;
    actual.retain(|path| path != MANIFEST_FILE);
    actual.sort();
    let expected_paths: Vec<_> = expected.keys().cloned().collect();
    if actual != expected_paths {
        bail!("backup contents do not match the manifest");
    }
    Ok(manifest)
}

pub(super) fn file_record(path: &Path, relative: &Path) -> Result<BackupFile> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let size_bytes = file.metadata()?.len();
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(BackupFile {
        path: slash_path(relative)?,
        size_bytes,
        sha256: hex::encode(hasher.finalize()),
    })
}

fn collect_relative_files(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_relative_files(root, &entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(slash_path(entry.path().strip_prefix(root)?)?);
        } else {
            bail!("backup contains a symlink or special file");
        }
    }
    Ok(())
}

pub(super) fn safe_join(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("unsafe path in backup manifest: {relative:?}");
    }
    Ok(root.join(relative))
}

pub(super) fn slash_path(path: &Path) -> Result<String> {
    let parts: Result<Vec<_>> = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .context("backup paths must be valid UTF-8"),
            _ => bail!("backup path must be relative and normalized"),
        })
        .collect();
    Ok(parts?.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_manifests_default_to_complete_file_scope() {
        let manifest: BackupManifest = serde_json::from_value(serde_json::json!({
            "format_version": 1,
            "created_at": "2026-08-26T00:00:00Z",
            "database_kind": "postgres",
            "dump_file": "database.dump",
            "attachments_directory": "attachments",
            "files": []
        }))
        .unwrap();
        assert!(manifest.includes_files);
    }

    #[test]
    fn safe_join_rejects_parent_paths() {
        assert!(safe_join(Path::new("/tmp/root"), "../secret").is_err());
        assert!(safe_join(Path::new("/tmp/root"), "/secret").is_err());
    }
}
