//! The client's view of a stored attachment. Its URLs are capability URLs: the download and,
//! for raster images, the TG-1302 thumbnail share one access key, so they are built together
//! here instead of at every row-to-payload conversion.

use uuid::Uuid;

use crate::models::Attachment;

pub(crate) struct AttachmentRow {
    pub id: Uuid,
    pub access_key: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub is_sensitive: bool,
}

impl AttachmentRow {
    pub(crate) fn into_attachment(self) -> Attachment {
        let Self {
            id,
            access_key,
            file_name,
            mime_type,
            size_bytes,
            is_sensitive,
        } = self;
        Attachment {
            id,
            thumbnail_url: super::thumbnails::thumbnail_url(id, &mime_type, access_key),
            download_url: format!("/api/attachments/{id}?key={access_key}"),
            file_name,
            mime_type,
            size_bytes,
            is_sensitive,
        }
    }
}
