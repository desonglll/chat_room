use std::{ffi::OsStr, fs, path::Path, process::Stdio};

use anyhow::{bail, Context, Result};
use tokio::process::Command;

use super::{
    files::{
        absolute_normalized, copy_attachment_tree, replace_attachment_files, sibling_temp_path,
        DirectoryCleanup,
    },
    package::{
        create_manifest, file_record, read_and_verify, BackupManifest, ATTACHMENTS_DIR, DUMP_FILE,
    },
    RestoreOutcome,
};
use crate::{cache::RedisCache, config::AppConfig};

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
    create_manifest(staging, "postgres", includes_files, files)
}

pub async fn restore_postgres(
    config: &AppConfig,
    database_url: &str,
    input: &Path,
) -> Result<RestoreOutcome> {
    let input = absolute_normalized(input)?;
    let manifest = read_and_verify(&input)?;
    if manifest.database_kind != "postgres" {
        bail!("backup database kind does not match PostgreSQL");
    }
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
        previous_database: None,
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
