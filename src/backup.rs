//! Complete PostgreSQL and local-attachment backup/restore commands.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::process::Command;
use uuid::Uuid;

use crate::{cache::SessionCache, config::AppConfig};

const FORMAT_VERSION: u32 = 1;
const DUMP_FILE: &str = "database.dump";
const ATTACHMENTS_DIR: &str = "attachments";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub format_version: u32,
    pub created_at: DateTime<Utc>,
    pub database_kind: String,
    pub dump_file: String,
    pub attachments_directory: String,
    pub files: Vec<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug)]
pub struct RestoreOutcome {
    pub previous_attachments: Option<PathBuf>,
    pub redis_keys_cleared: usize,
}

struct DirectoryCleanup {
    path: PathBuf,
    active: bool,
}

impl DirectoryCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for DirectoryCleanup {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub async fn export_postgres(
    config: &AppConfig,
    database_url: &str,
    output: &Path,
) -> Result<BackupManifest> {
    validate_local_backup_config(config)?;
    let output = absolute_normalized(output)?;
    let attachments = absolute_normalized(&config.attachments.directory)?;
    if output.starts_with(&attachments) {
        bail!("backup output must not be inside the attachment directory");
    }
    if output.exists() {
        bail!("backup output already exists: {}", output.display());
    }
    let parent = output
        .parent()
        .context("backup output must have a parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create backup parent {}", parent.display()))?;
    let staging = sibling_temp_path(&output, "export")?;
    fs::create_dir(&staging)
        .with_context(|| format!("create backup staging {}", staging.display()))?;
    let mut cleanup = DirectoryCleanup::new(staging.clone());

    let result = export_into(database_url, &attachments, &staging).await;
    match result {
        Ok(manifest) => {
            fs::rename(&staging, &output)
                .with_context(|| format!("publish backup at {}", output.display()))?;
            cleanup.disarm();
            Ok(manifest)
        }
        Err(error) => Err(error),
    }
}

async fn export_into(
    database_url: &str,
    attachments: &Path,
    staging: &Path,
) -> Result<BackupManifest> {
    let dump = staging.join(DUMP_FILE);
    run_command(
        "pg_dump",
        [
            OsStr::new("--format=custom"),
            OsStr::new("--file"),
            dump.as_os_str(),
            OsStr::new("--dbname"),
            OsStr::new(database_url),
        ],
    )
    .await
    .context("create PostgreSQL dump")?;

    let mut files = vec![file_record(&dump, Path::new(DUMP_FILE))?];
    let attachment_output = staging.join(ATTACHMENTS_DIR);
    fs::create_dir(&attachment_output).context("create attachment backup directory")?;
    if attachments.exists() {
        copy_tree(
            attachments,
            &attachment_output,
            Path::new(ATTACHMENTS_DIR),
            &mut files,
        )?;
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = BackupManifest {
        format_version: FORMAT_VERSION,
        created_at: Utc::now(),
        database_kind: "postgres".into(),
        dump_file: DUMP_FILE.into(),
        attachments_directory: ATTACHMENTS_DIR.into(),
        files,
    };
    let json = serde_json::to_vec_pretty(&manifest).context("encode backup manifest")?;
    File::create(staging.join(MANIFEST_FILE))
        .context("create backup manifest")?
        .write_all(&json)
        .context("write backup manifest")?;
    Ok(manifest)
}

pub async fn restore_postgres(
    config: &AppConfig,
    database_url: &str,
    input: &Path,
) -> Result<RestoreOutcome> {
    validate_local_backup_config(config)?;
    let input = absolute_normalized(input)?;
    let manifest = read_and_verify(&input)?;
    let attachment_source = safe_join(&input, &manifest.attachments_directory)?;
    let target = absolute_normalized(&config.attachments.directory)?;
    if target.starts_with(&input) || input.starts_with(&target) {
        bail!("backup input and attachment directory must not contain one another");
    }

    let target_parent = target
        .parent()
        .context("attachment directory must have a parent")?;
    fs::create_dir_all(target_parent)
        .with_context(|| format!("create attachment parent {}", target_parent.display()))?;
    let staged_attachments = sibling_temp_path(&target, "restore")?;
    fs::create_dir(&staged_attachments).context("create attachment restore staging")?;
    let mut cleanup = DirectoryCleanup::new(staged_attachments.clone());
    let mut ignored_records = Vec::new();
    if attachment_source.exists() {
        copy_tree(
            &attachment_source,
            &staged_attachments,
            Path::new(ATTACHMENTS_DIR),
            &mut ignored_records,
        )?;
    }

    let redis_cache = if config.redis.enabled {
        Some(SessionCache::connect(&config.redis).await.context(
            "connect to Redis before restore; disable Redis explicitly if it is intentionally unavailable",
        )?)
    } else {
        None
    };
    let redis_keys_cleared = match redis_cache {
        Some(cache) => cache
            .clear_all()
            .await
            .context("clear Redis cache before restore")?,
        None => 0,
    };
    let dump = safe_join(&input, &manifest.dump_file)?;
    let restore_result = run_command(
        "pg_restore",
        [
            OsStr::new("--clean"),
            OsStr::new("--if-exists"),
            OsStr::new("--no-owner"),
            OsStr::new("--no-privileges"),
            OsStr::new("--exit-on-error"),
            OsStr::new("--dbname"),
            OsStr::new(database_url),
            dump.as_os_str(),
        ],
    )
    .await
    .context("restore PostgreSQL dump");
    restore_result?;

    let previous_attachments = swap_attachment_directory(&target, &staged_attachments)?;
    cleanup.disarm();
    Ok(RestoreOutcome {
        previous_attachments,
        redis_keys_cleared,
    })
}

fn validate_local_backup_config(config: &AppConfig) -> Result<()> {
    if config.attachments.oss.enabled {
        bail!("complete local backup is unavailable while attachments.oss.enabled is true");
    }
    Ok(())
}

fn read_and_verify(root: &Path) -> Result<BackupManifest> {
    let bytes = fs::read(root.join(MANIFEST_FILE))
        .with_context(|| format!("read backup manifest from {}", root.display()))?;
    let manifest: BackupManifest =
        serde_json::from_slice(&bytes).context("decode backup manifest")?;
    if manifest.format_version != FORMAT_VERSION || manifest.database_kind != "postgres" {
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

fn copy_tree(
    source: &Path,
    target: &Path,
    prefix: &Path,
    records: &mut Vec<BackupFile>,
) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("read directory {}", source.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        let relative = prefix.join(entry.file_name());
        if file_type.is_dir() {
            fs::create_dir(&destination)
                .with_context(|| format!("create directory {}", destination.display()))?;
            copy_tree(&entry.path(), &destination, &relative, records)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &destination)
                .with_context(|| format!("copy attachment {}", entry.path().display()))?;
            records.push(file_record(&destination, &relative)?);
        } else {
            bail!(
                "attachment directory contains unsupported symlink or special file: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn file_record(path: &Path, relative: &Path) -> Result<BackupFile> {
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

fn swap_attachment_directory(target: &Path, staged: &Path) -> Result<Option<PathBuf>> {
    let previous = if target.exists() {
        let previous = sibling_temp_path(target, "pre-restore")?;
        fs::rename(target, &previous).with_context(|| format!("preserve {}", target.display()))?;
        Some(previous)
    } else {
        None
    };
    if let Err(error) = fs::rename(staged, target) {
        if let Some(previous) = &previous {
            let _ = fs::rename(previous, target);
        }
        return Err(error)
            .with_context(|| format!("activate restored attachments at {}", target.display()));
    }
    Ok(previous)
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
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

fn slash_path(path: &Path) -> Result<String> {
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

fn absolute_normalized(path: &Path) -> Result<PathBuf> {
    let source = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut result = PathBuf::new();
    for component in source.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(component)
            }
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
        }
    }
    Ok(result)
}

fn sibling_temp_path(path: &Path, label: &str) -> Result<PathBuf> {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .context("path must have a UTF-8 file name")?;
    Ok(path.with_file_name(format!(".{name}.{label}-{}", Uuid::new_v4().simple())))
}

async fn run_command<const N: usize>(program: &str, arguments: [&OsStr; N]) -> Result<()> {
    let output = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .await
        .with_context(|| {
            format!("start {program}; ensure PostgreSQL client tools are installed")
        })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("{program} failed with {}: {}", output.status, stderr.trim());
}
