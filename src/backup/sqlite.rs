use std::{fs, path::Path};

use anyhow::{bail, Context, Result};
use sqlx::SqlitePool;

use super::{
    files::{
        absolute_normalized, copy_attachment_tree, replace_attachment_files, sibling_temp_path,
        DirectoryCleanup,
    },
    package::{create_manifest, file_record, read_and_verify, ATTACHMENTS_DIR, DUMP_FILE},
    RestoreOutcome,
};
use crate::{cache::RedisCache, config::AppConfig};

pub async fn export_sqlite_scoped(
    config: &AppConfig,
    pool: &SqlitePool,
    output: &Path,
    includes_files: bool,
) -> Result<super::BackupManifest> {
    validate_config(config, includes_files)?;
    let output = absolute_normalized(output)?;
    if output.exists() {
        bail!("backup output already exists: {}", output.display());
    }
    let parent = output
        .parent()
        .context("backup output must have a parent")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create backup parent {}", parent.display()))?;
    let staging = sibling_temp_path(&output, "export")?;
    fs::create_dir(&staging)
        .with_context(|| format!("create backup staging {}", staging.display()))?;
    let mut cleanup = DirectoryCleanup::new(staging.clone());

    let dump = staging.join(DUMP_FILE);
    sqlx::query("VACUUM INTO ?")
        .bind(dump.to_string_lossy().as_ref())
        .execute(pool)
        .await
        .context("create online consistent SQLite snapshot")?;
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
    let manifest = create_manifest(&staging, "sqlite", includes_files, files)?;
    fs::rename(&staging, &output)
        .with_context(|| format!("publish backup at {}", output.display()))?;
    cleanup.disarm();
    Ok(manifest)
}

pub async fn restore_sqlite(config: &AppConfig, input: &Path) -> Result<RestoreOutcome> {
    let input = absolute_normalized(input)?;
    let manifest = read_and_verify(&input)?;
    if manifest.database_kind != "sqlite" {
        bail!("backup database kind does not match SQLite");
    }
    validate_config(config, manifest.includes_files)?;
    let target = absolute_normalized(&config.database.sqlite_path)?;
    let staged = sibling_temp_path(&target, "restore")?;
    fs::copy(input.join(&manifest.dump_file), &staged)
        .with_context(|| format!("stage restored SQLite database at {}", staged.display()))?;
    let previous = sibling_temp_path(&target, "pre-restore")?;
    fs::rename(&target, &previous).context("preserve current SQLite database")?;
    if let Err(error) = fs::rename(&staged, &target) {
        let _ = fs::rename(&previous, &target);
        return Err(error).context("activate restored SQLite database");
    }
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", target.display()));
    }

    let previous_attachments = if manifest.includes_files {
        Some(replace_attachment_files(
            &absolute_normalized(&config.attachments.directory)?,
            &input.join(&manifest.attachments_directory),
        )?)
    } else {
        None
    };
    let redis_keys_cleared = if config.redis.enabled {
        RedisCache::connect(&config.redis)
            .await
            .context("connect to Redis before restore")?
            .clear_all()
            .await
            .context("clear Redis cache after restore")?
    } else {
        0
    };
    Ok(RestoreOutcome {
        previous_database: Some(previous),
        previous_attachments,
        redis_keys_cleared,
        includes_files: manifest.includes_files,
    })
}

fn validate_config(config: &AppConfig, includes_files: bool) -> Result<()> {
    if includes_files && config.attachments.oss.enabled {
        bail!("file backup is unavailable while attachments.oss.enabled is true");
    }
    Ok(())
}
