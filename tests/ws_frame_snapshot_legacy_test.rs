//! Serialization snapshots for every pre-TG-007 WebSocket frame.
//!
//! TG-007 was the one deliberate breaking window for the WS protocol. Everything in this file
//! is pinned to serialise exactly as it did at the TG-007 base commit (d12aae2): if one of
//! these assertions moves, a frozen client (Vue, PySide6, ratatui) breaks. The frames TG-007
//! deliberately extended (`typing`, `auth_ok`) and the new frames live in
//! `ws_frame_snapshot_extension_test.rs`.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use chat_room::models::{
    Attachment, ChatMember, ChatMessage, ForwardedFrom, MessageReaction, ReplyPreview,
};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn when() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn member(n: u128, name: &str) -> ChatMember {
    ChatMember {
        user_id: id(n),
        username: name.into(),
        avatar_emoji: "🦀".into(),
    }
}

fn serialized(message: &ChatMessage) -> serde_json::Value {
    serde_json::to_value(message).unwrap()
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
        ChatMessage::Message { content, reply_to: None, client_message_id: None } if content == "hi"
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

#[test]
fn lifecycle_frames_serialize_unchanged() {
    assert_eq!(
        serialized(&ChatMessage::HistoryComplete),
        json!({ "type": "history_complete" })
    );
    assert_eq!(
        serialized(&ChatMessage::AuthFail {
            reason: "wrong password".into()
        }),
        json!({ "type": "auth_fail", "reason": "wrong password" })
    );
}

#[test]
fn broadcast_frame_serializes_unchanged() {
    let full = ChatMessage::Broadcast {
        message_id: id(10),
        client_message_id: Some(id(11)),
        sender_id: Some(id(12)),
        sender: "alice".into(),
        sender_avatar: "🦀".into(),
        content: "hello".into(),
        attachment: Some(Attachment {
            id: id(13),
            file_name: "cat.png".into(),
            mime_type: "image/png".into(),
            size_bytes: 42,
            download_url: "/api/attachments/x".into(),
            is_sensitive: false,
        }),
        reply_to: Some(ReplyPreview {
            message_id: id(14),
            sender: "bob".into(),
            content: "earlier".into(),
            attachment_file_name: None,
            recalled: false,
        }),
        recalled_at: None,
        edited_at: Some(when()),
        timestamp: when(),
        favorite_id: None,
        forwarded_from: Some(ForwardedFrom {
            sender: "carol".into(),
            room_name: "origin".into(),
        }),
        reactions: vec![MessageReaction {
            emoji: "👍".into(),
            user_ids: vec![id(12)],
        }],
    };
    assert_eq!(
        serialized(&full),
        json!({
            "type": "broadcast",
            "message_id": id(10),
            "client_message_id": id(11),
            "sender_id": id(12),
            "sender": "alice",
            "sender_avatar": "🦀",
            "content": "hello",
            "attachment": { "id": id(13), "file_name": "cat.png", "mime_type": "image/png",
                "size_bytes": 42, "download_url": "/api/attachments/x", "is_sensitive": false },
            "reply_to": { "message_id": id(14), "sender": "bob", "content": "earlier",
                "attachment_file_name": null, "recalled": false },
            "recalled_at": null,
            "edited_at": "2026-09-30T12:00:00Z",
            "timestamp": "2026-09-30T12:00:00Z",
            "favorite_id": null,
            "forwarded_from": { "sender": "carol", "room_name": "origin" },
            "reactions": [ { "emoji": "👍", "user_ids": [id(12)] } ]
        })
    );

    // `client_message_id` is omitted (not null) when absent — the frozen clients rely on it.
    let minimal = serialized(&ChatMessage::Broadcast {
        message_id: id(10),
        client_message_id: None,
        sender_id: None,
        sender: "alice".into(),
        sender_avatar: String::new(),
        content: "hello".into(),
        attachment: None,
        reply_to: None,
        recalled_at: None,
        edited_at: None,
        timestamp: when(),
        favorite_id: None,
        forwarded_from: None,
        reactions: Vec::new(),
    });
    assert!(minimal.get("client_message_id").is_none());
}

#[test]
fn edit_recall_reaction_frames_serialize_unchanged() {
    assert_eq!(
        serialized(&ChatMessage::MessageEdited {
            message_id: id(2),
            content: "new".into(),
            edited_at: when(),
        }),
        json!({ "type": "message_edited", "message_id": id(2), "content": "new",
            "edited_at": "2026-09-30T12:00:00Z" })
    );
    assert_eq!(
        serialized(&ChatMessage::MessageRecalled {
            message_id: id(2),
            recalled_at: when(),
        }),
        json!({ "type": "message_recalled", "message_id": id(2),
            "recalled_at": "2026-09-30T12:00:00Z" })
    );
    assert_eq!(
        serialized(&ChatMessage::ReactionChanged {
            message_id: id(2),
            emoji: "👍".into(),
            user_id: id(3),
            active: false,
        }),
        json!({ "type": "reaction_changed", "message_id": id(2), "emoji": "👍",
            "user_id": id(3), "active": false })
    );
}

#[test]
fn presence_read_receipt_and_system_frames_serialize_unchanged() {
    // TG-007 deliberately left `presence` and `ChatMember` alone: per-user status travels in
    // the new `user_status` frame instead (docs/devlog/TG-007.md, Frozen interface §2).
    assert_eq!(
        serialized(&ChatMessage::Presence {
            members: vec![member(3, "alice")],
            participants: vec![member(3, "alice"), member(4, "bob")],
        }),
        json!({
            "type": "presence",
            "members": [ { "user_id": id(3), "username": "alice", "avatar_emoji": "🦀" } ],
            "participants": [
                { "user_id": id(3), "username": "alice", "avatar_emoji": "🦀" },
                { "user_id": id(4), "username": "bob", "avatar_emoji": "🦀" }
            ]
        })
    );
    assert_eq!(
        serialized(&ChatMessage::ReadReceipt {
            user_id: id(3),
            username: "alice".into(),
            message_id: id(10),
        }),
        json!({ "type": "read_receipt", "user_id": id(3), "username": "alice",
            "message_id": id(10) })
    );
    assert_eq!(
        serialized(&ChatMessage::System {
            content: "alice joined the chat".into(),
            members: Some(vec![member(3, "alice")]),
            participants: None,
        }),
        json!({
            "type": "system",
            "content": "alice joined the chat",
            "members": [ { "user_id": id(3), "username": "alice", "avatar_emoji": "🦀" } ]
        })
    );
    assert_eq!(
        serialized(&ChatMessage::System {
            content: "session revoked".into(),
            members: None,
            participants: None,
        }),
        json!({ "type": "system", "content": "session revoked" })
    );
}
