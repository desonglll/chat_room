//! Deserialization snapshots for every pre-TG-007 client→server WebSocket frame.
//!
//! Split out of `ws_frame_snapshot_legacy_test.rs` (file-size limit) — same contract: a
//! frozen client (Vue, PySide6, ratatui) still sends exactly these shapes.

use serde_json::json;
use uuid::Uuid;

use chat_room::models::ChatMessage;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

#[test]
fn client_to_server_frames_deserialize_unchanged() {
    let join: ChatMessage =
        serde_json::from_value(json!({ "type": "join", "token": id(1) })).unwrap();
    assert!(matches!(join, ChatMessage::Join { token } if token == id(1)));

    let auth: ChatMessage =
        serde_json::from_value(json!({ "type": "auth", "token": id(1), "password": "pw" }))
            .unwrap();
    assert!(matches!(auth, ChatMessage::Auth { password, .. } if password == "pw"));

    let message: ChatMessage =
        serde_json::from_value(json!({ "type": "message", "content": "hi" })).unwrap();
    assert!(matches!(
        message,
        ChatMessage::Message { content, reply_to: None, client_message_id: None, .. } if content == "hi"
    ));

    let edit: ChatMessage =
        serde_json::from_value(json!({ "type": "edit", "message_id": id(2), "content": "x" }))
            .unwrap();
    assert!(matches!(edit, ChatMessage::Edit { message_id, .. } if message_id == id(2)));

    let read: ChatMessage =
        serde_json::from_value(json!({ "type": "read", "message_id": id(2) })).unwrap();
    assert!(matches!(read, ChatMessage::Read { message_id } if message_id == id(2)));

    let recall: ChatMessage =
        serde_json::from_value(json!({ "type": "recall", "message_id": id(2) })).unwrap();
    assert!(matches!(recall, ChatMessage::Recall { message_id } if message_id == id(2)));

    let reaction: ChatMessage = serde_json::from_value(
        json!({ "type": "reaction", "message_id": id(2), "emoji": "👍", "active": true }),
    )
    .unwrap();
    assert!(matches!(
        reaction,
        ChatMessage::Reaction { active: true, .. }
    ));

    let poke: ChatMessage =
        serde_json::from_value(json!({ "type": "poke", "target_user_id": id(3) })).unwrap();
    assert!(matches!(poke, ChatMessage::Poke { target_user_id } if target_user_id == id(3)));
}
