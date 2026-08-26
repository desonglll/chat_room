use std::{
    fs::{self, File},
    path::{Component, Path},
};

use anyhow::{bail, Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use tar::{Archive, Builder, EntryType};

pub fn pack_archive(package: &Path, output: &Path) -> Result<()> {
    let file = File::create(output)
        .with_context(|| format!("create backup archive {}", output.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);
    archive
        .append_dir_all(".", package)
        .with_context(|| format!("archive backup package {}", package.display()))?;
    let encoder = archive.into_inner().context("finish backup archive")?;
    encoder.finish().context("finish backup compression")?;
    Ok(())
}

pub fn unpack_archive(input: &Path, output: &Path) -> Result<()> {
    fs::create_dir_all(output)
        .with_context(|| format!("create archive output {}", output.display()))?;
    let file =
        File::open(input).with_context(|| format!("open backup archive {}", input.display()))?;
    let mut archive = Archive::new(GzDecoder::new(file));
    for entry in archive.entries().context("read backup archive")? {
        let mut entry = entry.context("read backup archive entry")?;
        let kind = entry.header().entry_type();
        if !matches!(kind, EntryType::Regular | EntryType::Directory) {
            bail!("backup archive contains a link or special file");
        }
        let path = entry.path().context("read backup archive path")?;
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::CurDir | Component::Normal(_)))
        {
            bail!("backup archive contains an unsafe path");
        }
        if !entry
            .unpack_in(output)
            .context("extract backup archive entry")?
        {
            bail!("backup archive entry escaped the output directory");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn archive_round_trip_preserves_package_files() {
        let root = std::env::temp_dir().join(format!("chat-backup-{}", Uuid::new_v4()));
        let package = root.join("package");
        let extracted = root.join("extracted");
        fs::create_dir_all(package.join("attachments/aa")).unwrap();
        fs::write(package.join("database.dump"), b"database").unwrap();
        fs::write(package.join("attachments/aa/file"), b"file").unwrap();
        let archive = root.join("backup.tar.gz");

        pack_archive(&package, &archive).unwrap();
        unpack_archive(&archive, &extracted).unwrap();

        assert_eq!(
            fs::read(extracted.join("database.dump")).unwrap(),
            b"database"
        );
        assert_eq!(
            fs::read(extracted.join("attachments/aa/file")).unwrap(),
            b"file"
        );
        let _ = fs::remove_dir_all(root);
    }
}
