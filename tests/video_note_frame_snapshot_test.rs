//! TG-402 wire pin: the `broadcast` frame's optional `video_note`, and `StoredMessage`'s.
//! The web client's `@tg/core` `VideoNote` mirrors exactly this shape.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use chat_room::attachments::video_note::model::VideoNote;
use chat_room::models::{ChatMessage, StoredMessage};

#[test]
fn broadcast_video_note_is_omitted_when_absent_and_shaped_when_present() {
    let when: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-10-01T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let broadcast = |video_note: Option<VideoNote>| ChatMessage::Broadcast {
        message_id: Uuid::from_u128(9),
        client_message_id: None,
        sender_id: Some(Uuid::from_u128(3)),
        sender: "alice".into(),
        sender_avatar: String::new(),
        content: String::new(),
        attachment: None,
        reply_to: None,
        recalled_at: None,
        edited_at: None,
        timestamp: when,
        favorite_id: None,
        forwarded_from: None,
        reactions: Vec::new(),
        media_kind: video_note.as_ref().map(|_| "video_note".to_string()),
        sticker: None,
        poll: None,
        entities: Vec::new(),
        voice: None,
        video_note,
        contact: None,
        location: None,
        topic_id: None,
        views: None,
        post_author: None,
        comments: None,
        silent: false,
        grouped_id: None,
    };
    let plain = serde_json::to_value(broadcast(None)).unwrap();
    assert!(plain.get("video_note").is_none());

    let round = serde_json::to_value(broadcast(Some(VideoNote {
        duration_ms: 12_345,
        thumbnail: Some("/9j/".into()),
        listened: false,
    })))
    .unwrap();
    assert_eq!(round["media_kind"], "video_note");
    assert_eq!(
        round["video_note"],
        json!({ "duration_ms": 12345, "thumbnail": "/9j/", "listened": false })
    );
    let bare = serde_json::to_value(broadcast(Some(VideoNote {
        duration_ms: 1,
        thumbnail: None,
        listened: true,
    })))
    .unwrap();
    assert_eq!(
        bare["video_note"],
        json!({ "duration_ms": 1, "thumbnail": null, "listened": true })
    );

    let stored = serde_json::to_value(StoredMessage::default()).unwrap();
    assert!(stored.get("video_note").is_none());
}
