//! TG-401 wire pins: the `voice_listened` frame and the `broadcast` frame's optional `voice`.
//! The web client's `@tg/core` types mirror exactly these shapes.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use chat_room::attachments::voice::model::VoiceNote;
use chat_room::models::ChatMessage;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

#[test]
fn voice_listened_serializes_as_frozen() {
    let frame = ChatMessage::VoiceListened {
        message_id: id(1),
        user_id: id(2),
        sender_id: Some(id(3)),
    };
    assert_eq!(
        serde_json::to_string(&frame).unwrap(),
        concat!(
            r#"{"type":"voice_listened","message_id":"00000000-0000-0000-0000-000000000001","#,
            r#""user_id":"00000000-0000-0000-0000-000000000002","#,
            r#""sender_id":"00000000-0000-0000-0000-000000000003"}"#
        )
    );
    let anonymous_sender = ChatMessage::VoiceListened {
        message_id: id(1),
        user_id: id(2),
        sender_id: None,
    };
    assert_eq!(
        serde_json::to_value(&anonymous_sender).unwrap()["sender_id"],
        json!(null)
    );
}

#[test]
fn broadcast_voice_is_omitted_when_absent_and_shaped_when_present() {
    let when: DateTime<Utc> = DateTime::parse_from_rfc3339("2026-10-01T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let broadcast = |voice: Option<VoiceNote>| ChatMessage::Broadcast {
        message_id: id(9),
        client_message_id: None,
        sender_id: Some(id(3)),
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
        media_kind: voice.as_ref().map(|_| "voice".to_string()),
        sticker: None,
        poll: None,
        entities: Vec::new(),
        voice,
        silent: false,
    };
    let plain = serde_json::to_value(broadcast(None)).unwrap();
    assert!(plain.get("voice").is_none());
    assert!(plain.get("media_kind").is_none());

    let voiced = serde_json::to_value(broadcast(Some(VoiceNote {
        duration_ms: 1_234,
        waveform: vec![31; 100],
        listened: false,
    })))
    .unwrap();
    assert_eq!(voiced["media_kind"], "voice");
    assert_eq!(
        voiced["voice"],
        json!({ "duration_ms": 1234, "waveform": vec![31; 100], "listened": false })
    );
}
