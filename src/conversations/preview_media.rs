//! TG-802: what kind of message a one-line preview stands for (chat list row, account feed).
//!
//! The client used to guess from the attachment's file extension, which called a voice
//! message "audio" and could not see polls, locations or contact cards at all. The server
//! knows: `messages.media_kind` for kinds with a dedicated send path, side tables for polls
//! and locations, `grouped_id` for albums and the MIME type for plain uploads.

/// SQL columns a preview query selects so [`preview_media_kind`] can classify the row.
/// `$m` is the message alias, `$a` the attachment alias.
macro_rules! preview_media_columns {
    ($m:literal, $a:literal) => {
        concat!(
            $m,
            ".media_kind AS preview_media_kind, ",
            $a,
            ".mime_type AS preview_mime_type, ",
            $m,
            ".grouped_id IS NOT NULL AS preview_in_album, ",
            "EXISTS (SELECT 1 FROM polls WHERE polls.message_id = ",
            $m,
            ".id) AS preview_is_poll, ",
            "(SELECT locations.live_until IS NOT NULL FROM message_locations AS locations \
             WHERE locations.message_id = ",
            $m,
            ".id) AS preview_live_location"
        )
    };
}
pub(crate) use preview_media_columns;

/// The raw columns from [`preview_media_columns`].
#[derive(Debug, Default, Clone, sqlx::FromRow)]
pub(crate) struct PreviewMediaColumns {
    pub preview_media_kind: Option<String>,
    pub preview_mime_type: Option<String>,
    pub preview_in_album: Option<bool>,
    pub preview_is_poll: Option<bool>,
    pub preview_live_location: Option<bool>,
}

/// One of `voice`, `video_note`, `sticker`, `gif`, `poll`, `location`, `live_location`,
/// `contact`, `album`, `photo`, `video`, `audio`, `file`; `None` for a plain text message.
pub(crate) fn preview_media_kind(columns: &PreviewMediaColumns) -> Option<&'static str> {
    if columns.preview_is_poll == Some(true) {
        return Some("poll");
    }
    match columns.preview_media_kind.as_deref() {
        Some("voice") => return Some("voice"),
        Some("video_note") => return Some("video_note"),
        Some("sticker") => return Some("sticker"),
        Some("gif") => return Some("gif"),
        Some("contact") => return Some("contact"),
        Some("location") => {
            return Some(if columns.preview_live_location == Some(true) {
                "live_location"
            } else {
                "location"
            })
        }
        _ => {}
    }
    let mime = columns.preview_mime_type.as_deref()?;
    let by_mime = match mime.split_once('/').map(|(kind, _)| kind) {
        _ if mime == "image/gif" => "gif",
        Some("image") => "photo",
        Some("video") => "video",
        Some("audio") => "audio",
        _ => "file",
    };
    if columns.preview_in_album == Some(true) && matches!(by_mime, "photo" | "video") {
        return Some("album");
    }
    Some(by_mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(media_kind: Option<&str>, mime: Option<&str>) -> Option<&'static str> {
        preview_media_kind(&PreviewMediaColumns {
            preview_media_kind: media_kind.map(str::to_string),
            preview_mime_type: mime.map(str::to_string),
            ..PreviewMediaColumns::default()
        })
    }

    #[test]
    fn dedicated_kinds_win_over_the_mime_type() {
        assert_eq!(kind(Some("voice"), Some("audio/ogg")), Some("voice"));
        assert_eq!(
            kind(Some("video_note"), Some("video/mp4")),
            Some("video_note")
        );
        assert_eq!(kind(Some("sticker"), Some("image/webp")), Some("sticker"));
        assert_eq!(kind(Some("gif"), Some("video/mp4")), Some("gif"));
        assert_eq!(kind(Some("contact"), None), Some("contact"));
        assert_eq!(kind(Some("location"), None), Some("location"));
    }

    #[test]
    fn plain_uploads_are_classified_by_mime_type() {
        assert_eq!(kind(None, None), None);
        assert_eq!(kind(None, Some("image/png")), Some("photo"));
        assert_eq!(kind(None, Some("image/gif")), Some("gif"));
        assert_eq!(kind(None, Some("video/mp4")), Some("video"));
        assert_eq!(kind(None, Some("audio/mpeg")), Some("audio"));
        assert_eq!(kind(None, Some("application/pdf")), Some("file"));
    }

    #[test]
    fn polls_live_locations_and_albums_come_from_side_columns() {
        let poll = PreviewMediaColumns {
            preview_is_poll: Some(true),
            ..PreviewMediaColumns::default()
        };
        assert_eq!(preview_media_kind(&poll), Some("poll"));
        let live = PreviewMediaColumns {
            preview_media_kind: Some("location".into()),
            preview_live_location: Some(true),
            ..PreviewMediaColumns::default()
        };
        assert_eq!(preview_media_kind(&live), Some("live_location"));
        let album = |mime: &str| PreviewMediaColumns {
            preview_mime_type: Some(mime.into()),
            preview_in_album: Some(true),
            ..PreviewMediaColumns::default()
        };
        assert_eq!(preview_media_kind(&album("image/jpeg")), Some("album"));
        assert_eq!(preview_media_kind(&album("application/pdf")), Some("file"));
    }
}
