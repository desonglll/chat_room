use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use uuid::Uuid;

use super::package::{file_record, BackupFile};

pub(super) struct DirectoryCleanup {
    path: PathBuf,
    active: bool,
}

impl DirectoryCleanup {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    pub(super) fn disarm(&mut self) {
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

pub(super) fn copy_attachment_tree(
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

pub(super) fn replace_attachment_files(target: &Path, source: &Path) -> Result<PathBuf> {
    fs::create_dir_all(target)
        .with_context(|| format!("create attachment directory {}", target.display()))?;
    let previous = target.join(format!(".pre-restore-{}", Uuid::new_v4().simple()));
    fs::create_dir(&previous).context("create previous attachment directory")?;

    if let Err(error) = move_visible_entries(target, &previous, Some(&previous)) {
        let _ = move_visible_entries(&previous, target, None);
        return Err(error).context("preserve current attachments");
    }
    if let Err(error) = move_visible_entries(source, target, None) {
        let _ = move_visible_entries(target, source, None);
        let _ = move_visible_entries(&previous, target, None);
        return Err(error).context("activate restored attachments");
    }
    Ok(previous)
}

pub(super) fn absolute_normalized(path: &Path) -> Result<PathBuf> {
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

pub(super) fn sibling_temp_path(path: &Path, label: &str) -> Result<PathBuf> {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .context("path must have a UTF-8 file name")?;
    Ok(path.with_file_name(format!(".{name}.{label}-{}", Uuid::new_v4().simple())))
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
