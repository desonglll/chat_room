//! Attachment upload, download, indexing, and terminal summaries for the CLI.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use uuid::Uuid;

pub const MAX_UPLOAD_BYTES: u64 = 50 * 1024 * 1024;
#[derive(Clone, Debug, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub download_url: String,
}

#[derive(Deserialize)]
struct UploadedMessage {
    attachment: Option<Attachment>,
}

pub async fn upload(
    http_base: &str,
    room_id: Uuid,
    token: Uuid,
    password: Option<&str>,
    path: &Path,
) -> Result<Attachment> {
    let metadata = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("read file metadata for {}", path.display()))?;
    if !metadata.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if metadata.len() == 0 {
        bail!("file is empty");
    }
    if metadata.len() > MAX_UPLOAD_BYTES {
        bail!("file exceeds the 50 MiB upload limit");
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .context("file name is not valid UTF-8")?
        .to_string();
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("read {}", path.display()))?;
    let mime_type = mime_guess::from_path(path).first_or_octet_stream();
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime_type.as_ref())
        .context("prepare upload content type")?;
    let mut request = reqwest::Client::new()
        .post(format!("{http_base}/api/rooms/{room_id}/attachments"))
        .bearer_auth(token)
        .multipart(reqwest::multipart::Form::new().part("file", part));
    if let Some(password) = password {
        request = request.header("x-room-password", password);
    }
    let response = request.send().await.context("upload attachment")?;
    match response.status().as_u16() {
        201 => response
            .json::<UploadedMessage>()
            .await
            .context("decode uploaded attachment")?
            .attachment
            .context("server omitted uploaded attachment metadata"),
        400 => bail!("server rejected the file name or upload body"),
        401 => bail!("login expired or room password is incorrect"),
        404 => bail!("room no longer exists"),
        413 => bail!("file exceeds the 50 MiB upload limit"),
        status => bail!("upload returned unexpected status {status}"),
    }
}

pub async fn download(
    http_base: &str,
    attachment: &Attachment,
    destination: Option<&Path>,
) -> Result<PathBuf> {
    let path = destination
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&attachment.file_name));
    if path.exists() {
        bail!("refusing to overwrite existing file {}", path.display());
    }
    let response = reqwest::get(format!("{http_base}{}", attachment.download_url))
        .await
        .context("download attachment")?;
    if !response.status().is_success() {
        bail!("download returned {}", response.status());
    }
    let bytes = response.bytes().await.context("read attachment response")?;
    tokio::fs::write(&path, bytes)
        .await
        .with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

pub fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_attachment_sizes() {
        assert_eq!(format_size(1536), "1.5 KiB");
        assert_eq!(format_size(2 * 1024 * 1024), "2.0 MiB");
    }
}
