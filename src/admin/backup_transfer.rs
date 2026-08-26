use std::{
    io,
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::{Context as _, Result};
use axum::{
    body::{Body, Bytes},
    http::{header, HeaderValue},
    response::Response,
};
use futures_util::Stream;
use tokio_util::io::ReaderStream;

use crate::{backup, config::AppConfig};

pub(super) struct WorkDirectory {
    pub path: PathBuf,
    active: bool,
}

impl WorkDirectory {
    pub fn create(config: &AppConfig, label: &str) -> Result<Self> {
        backup::create_work_directory(config, label).map(|path| Self { path, active: true })
    }

    pub async fn download(self, archive: &Path, filename: &str) -> Result<Response<Body>> {
        let file = tokio::fs::File::open(archive)
            .await
            .with_context(|| format!("open backup archive {}", archive.display()))?;
        let size = file.metadata().await?.len();
        let work_directory = self.into_path();
        let stream = TemporaryArchiveStream {
            reader: ReaderStream::new(file),
            work_directory,
        };
        let mut response = Response::new(Body::from_stream(stream));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(backup::ARCHIVE_CONTENT_TYPE),
        );
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&size.to_string()).context("encode archive length")?,
        );
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                .context("encode backup filename")?,
        );
        Ok(response)
    }

    fn into_path(mut self) -> PathBuf {
        let path = self.path.clone();
        self.active = false;
        path
    }
}

impl Drop for WorkDirectory {
    fn drop(&mut self) {
        if self.active {
            backup::remove_work_directory(&self.path);
        }
    }
}

struct TemporaryArchiveStream {
    reader: ReaderStream<tokio::fs::File>,
    work_directory: PathBuf,
}

impl Stream for TemporaryArchiveStream {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.reader).poll_next(context)
    }
}

impl Drop for TemporaryArchiveStream {
    fn drop(&mut self) {
        backup::remove_work_directory(&self.work_directory);
    }
}
