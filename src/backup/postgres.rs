use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use tokio::process::Command;
use uuid::Uuid;

use super::package::{
    file_record, read_and_verify, BackupFile, BackupManifest, ATTACHMENTS_DIR, DUMP_FILE,
    FORMAT_VERSION, MANIFEST_FILE,
};
use crate::{cache::RedisCache, config::AppConfig};

#[derive(Debug)]
pub struct RestoreOutcome {
    pub previous_attachments: Option<PathBuf>,
    pub redis_keys_cleared: usize,
    pub includes_files: bool,
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
    let output = absolute_normalized(output)?;
    let attachments = absolute_normalized(&config.attachments.directory)?;
    if output.starts_with(attachments) {
        bail!("backup output must not be inside the attachment directory");
    }
    export_postgres_scoped(config, database_url, &output, true).await
}

pub async fn export_postgres_scoped(
    config: &AppConfig,
    database_url: &str,
    output: &Path,
    includes_files: bool,
) -> Result<BackupManifest> {
    validate_backup_config(config, includes_files)?;
    let output = absolute_normalized(output)?;
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

    let manifest = export_into(config, database_url, &staging, includes_files).await?;
    fs::rename(&staging, &output)
        .with_context(|| format!("publish backup at {}", output.display()))?;
    cleanup.disarm();
    Ok(manifest)
}

async fn export_into(
    config: &AppConfig,
    database_url: &str,
    staging: &Path,
    includes_files: bool,
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
    if includes_files {
        let attachment_output = staging.join(ATTACHMENTS_DIR);
        fs::create_dir(&attachment_output).context("create attachment backup directory")?;
        let attachments = absolute_normalized(&config.attachments.directory)?;
        if attachments.exists() {
            copy_attachment_tree(
                &attachments,
                &attachment_output,
                Path::new(ATTACHMENTS_DIR),
                true,
                &mut files,
            )?;
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = BackupManifest {
        format_version: FORMAT_VERSION,
        created_at: Utc::now(),
        database_kind: "postgres".into(),
        dump_file: DUMP_FILE.into(),
        attachments_directory: ATTACHMENTS_DIR.into(),
        includes_files,
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
    let input = absolute_normalized(input)?;
    let manifest = read_and_verify(&input)?;
    validate_backup_config(config, manifest.includes_files)?;

    let redis_cache = if config.redis.enabled {
        Some(RedisCache::connect(&config.redis).await.context(
            "connect to Redis before restore; disable Redis explicitly if it is intentionally unavailable",
        )?)
    } else {
        None
    };
    let dump = input.join(&manifest.dump_file);
    run_command(
        "pg_restore",
        [
            OsStr::new("--clean"),
            OsStr::new("--if-exists"),
            OsStr::new("--no-owner"),
            OsStr::new("--no-privileges"),
            OsStr::new("--exit-on-error"),
            OsStr::new("--single-transaction"),
            OsStr::new("--dbname"),
            OsStr::new(database_url),
            dump.as_os_str(),
        ],
    )
    .await
    .context("restore PostgreSQL dump")?;

    let previous_attachments = if manifest.includes_files {
        let source = input.join(&manifest.attachments_directory);
        Some(replace_attachment_files(
            &absolute_normalized(&config.attachments.directory)?,
            &source,
        )?)
    } else {
        None
    };
    let redis_keys_cleared = match redis_cache {
        Some(cache) => cache
            .clear_all()
            .await
            .context("clear Redis cache after restore")?,
        None => 0,
    };
    Ok(RestoreOutcome {
        previous_attachments,
        redis_keys_cleared,
        includes_files: manifest.includes_files,
    })
}

fn validate_backup_config(config: &AppConfig, includes_files: bool) -> Result<()> {
    if includes_files && config.attachments.oss.enabled {
        bail!("file backup is unavailable while attachments.oss.enabled is true");
    }
    Ok(())
}

fn copy_attachment_tree(
    source: &Path,
    target: &Path,
    prefix: &Path,
    top_level: bool,
    records: &mut Vec<BackupFile>,
) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("read directory {}", source.display()))?
    {
        let entry = entry?;
        if top_level && entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        let relative = prefix.join(entry.file_name());
        if file_type.is_dir() {
            fs::create_dir(&destination)
                .with_context(|| format!("create directory {}", destination.display()))?;
            copy_attachment_tree(&entry.path(), &destination, &relative, false, records)?;
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

fn replace_attachment_files(target: &Path, source: &Path) -> Result<PathBuf> {
    fs::create_dir_all(target)
        .with_context(|| format!("create attachment directory {}", target.display()))?;
    let previous = target.join(format!(".pre-restore-{}", Uuid::new_v4().simple()));
    fs::create_dir(&previous).context("create previous attachment directory")?;

    if let Err(error) = move_visible_entries(target, &previous, Some(&previous)) {
        let _ = move_visible_entries(&previous, target, None);
        return Err(error).context("preserve current attachments");
    }
    if let Err(error) = move_visible_entries(source, target, None) {
        // Return any already-activated files to the verified package before
        // putting the previous attachment set back in place.
        let _ = move_visible_entries(target, source, None);
        let _ = move_visible_entries(&previous, target, None);
        return Err(error).context("activate restored attachments");
    }
    Ok(previous)
}

fn move_visible_entries(source: &Path, target: &Path, skip: Option<&Path>) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.')
            || skip.is_some_and(|path| entry.path() == path)
        {
            continue;
        }
        fs::rename(entry.path(), target.join(entry.file_name()))?;
    }
    Ok(())
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
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => result.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
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
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = command.output().await.with_context(|| {
        format!("start {program}; ensure PostgreSQL client tools are installed")
    })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("{program} failed with {}: {}", output.status, stderr.trim());
}

#[cfg(test)]
mod tests;
