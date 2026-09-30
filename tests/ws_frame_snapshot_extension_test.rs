//! Serialization snapshots for the frames TG-007 extended (`typing`, `auth_ok`) and added
//! (`user_status`, `chat_updated`, `member_updated`, `topic_updated`, `message_views_updated`,
//! `poll_updated`, `draft_updated`), plus the compatibility rules for pre-TG-007 clients.
//! The shapes are frozen in docs/devlog/TG-007.md; later milestones extend additively only.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use chat_room::models::{
    Chat, ChatMembership, ChatMessage, MessageViewCount, PollOption, PollState, TopicSummary,
    TypingAction, UserStatus, UserStatusEntry,
};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn when() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-30T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn serialized(message: &ChatMessage) -> serde_json::Value {
    serde_json::to_value(message).unwrap()
}

/// Pins one server→client frame three ways, mirroring `ws_frame_snapshot_legacy_test.rs`:
/// the order-insensitive `Value` snapshot (the readable shape), the exact serialised string
/// (serde emits struct fields in declaration order, so a field reorder is a real wire change
/// the `Value` comparison alone cannot see), and the exact string parsed back against the
/// snapshot (so a typo in the literal cannot pin a wrong shape).
fn assert_wire(frame: &ChatMessage, snapshot: serde_json::Value, exact: &str) {
    assert_eq!(serialized(frame), snapshot);
    assert_eq!(serde_json::to_string(frame).unwrap(), exact);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(exact).unwrap(),
        snapshot
    );
}

#[test]
fn typing_carries_the_action_and_defaults_it_for_legacy_clients() {
    assert_wire(
        &ChatMessage::Typing {
            content: "dra".into(),
            action: TypingAction::RecordingVoice,
            user_id: Some(id(3)),
            username: Some("alice".into()),
        },
        json!({ "type": "typing", "content": "dra", "action": "recording_voice",
            "user_id": id(3), "username": "alice" }),
        concat!(
            r#"{"type":"typing","content":"dra","action":"recording_voice","#,
            r#""user_id":"00000000-0000-0000-0000-000000000003","username":"alice"}"#
        ),
    );

    // On output `user_id` / `username` are omitted — not null — when absent, per their
    // `skip_serializing_if = "Option::is_none"` attrs in `src/realtime/frames.rs`.
    assert_wire(
        &ChatMessage::Typing {
            content: String::new(),
            action: TypingAction::Cancel,
            user_id: None,
            username: None,
        },
        json!({ "type": "typing", "content": "", "action": "cancel" }),
        r#"{"type":"typing","content":"","action":"cancel"}"#,
    );

    // A pre-TG-007 client sends no action: that means plain typing.
    let legacy: ChatMessage =
        serde_json::from_value(json!({ "type": "typing", "content": "dra" })).unwrap();
    let ChatMessage::Typing {
        action, user_id, ..
    } = legacy
    else {
        panic!("expected typing");
    };
    assert_eq!(action, TypingAction::Typing);
    assert_eq!(user_id, None);

    // An action this server does not know yet degrades to typing instead of dropping the frame.
    let future: ChatMessage =
        serde_json::from_value(json!({ "type": "typing", "content": "", "action": "levitating" }))
            .unwrap();
    let ChatMessage::Typing { action, .. } = future else {
        panic!("expected typing");
    };
    assert_eq!(action, TypingAction::Typing);
}

#[test]
fn every_typing_action_round_trips_through_its_wire_name() {
    let actions = [
        (TypingAction::Typing, "typing"),
        (TypingAction::RecordingVoice, "recording_voice"),
        (TypingAction::RecordingVideoNote, "recording_video_note"),
        (TypingAction::UploadingPhoto, "uploading_photo"),
        (TypingAction::UploadingVideo, "uploading_video"),
        (TypingAction::UploadingDocument, "uploading_document"),
        (TypingAction::UploadingVoice, "uploading_voice"),
        (TypingAction::ChoosingSticker, "choosing_sticker"),
        (TypingAction::ChoosingLocation, "choosing_location"),
        (TypingAction::Cancel, "cancel"),
    ];
    for (action, wire) in actions {
        assert_eq!(serde_json::to_value(action).unwrap(), json!(wire));
        // The `From<String>` fallback must agree with the serialised names, or a rename
        // would silently turn a granular action into plain typing.
        assert_eq!(TypingAction::from(wire.to_string()), action);
    }
}

#[test]
fn auth_ok_gains_a_per_user_status_snapshot() {
    let frame = ChatMessage::AuthOk {
        room_name: "general".into(),
        members: Vec::new(),
        participants: Vec::new(),
        read_receipts: Vec::new(),
        statuses: vec![
            UserStatusEntry {
                user_id: id(3),
                status: UserStatus::Online,
            },
            UserStatusEntry {
                user_id: id(4),
                status: UserStatus::Empty,
            },
        ],
    };
    assert_wire(
        &frame,
        json!({
            "type": "auth_ok",
            "room_name": "general",
            "members": [], "participants": [], "read_receipts": [],
            "statuses": [
                { "user_id": id(3), "status": { "kind": "online" } },
                { "user_id": id(4), "status": { "kind": "empty" } }
            ]
        }),
        concat!(
            r#"{"type":"auth_ok","room_name":"general","members":[],"participants":[],"#,
            r#""read_receipts":[],"#,
            r#""statuses":[{"user_id":"00000000-0000-0000-0000-000000000003","#,
            r#""status":{"kind":"online"}},"#,
            r#"{"user_id":"00000000-0000-0000-0000-000000000004","#,
            r#""status":{"kind":"empty"}}]}"#
        ),
    );
}

#[test]
fn user_status_frame_serializes_every_tier() {
    assert_wire(
        &ChatMessage::UserStatusChanged {
            user_id: id(3),
            status: UserStatus::Offline { last_seen: when() },
        },
        json!({ "type": "user_status", "user_id": id(3),
            "status": { "kind": "offline", "last_seen": "2026-09-30T12:00:00Z" } }),
        concat!(
            r#"{"type":"user_status","user_id":"00000000-0000-0000-0000-000000000003","#,
            r#""status":{"kind":"offline","last_seen":"2026-09-30T12:00:00Z"}}"#
        ),
    );
    // The privacy tiers (TG-505's placeholders) and the two live kinds, each pinned to the
    // exact string as well: `kind` must stay the first (here only) key of the tagged object.
    let tiers = [
        (
            UserStatus::Online,
            json!({ "kind": "online" }),
            r#"{"kind":"online"}"#,
        ),
        (
            UserStatus::Recently,
            json!({ "kind": "recently" }),
            r#"{"kind":"recently"}"#,
        ),
        (
            UserStatus::WithinWeek,
            json!({ "kind": "within_week" }),
            r#"{"kind":"within_week"}"#,
        ),
        (
            UserStatus::WithinMonth,
            json!({ "kind": "within_month" }),
            r#"{"kind":"within_month"}"#,
        ),
        (
            UserStatus::LongAgo,
            json!({ "kind": "long_ago" }),
            r#"{"kind":"long_ago"}"#,
        ),
        (
            UserStatus::Empty,
            json!({ "kind": "empty" }),
            r#"{"kind":"empty"}"#,
        ),
    ];
    for (status, wire, exact) in tiers {
        assert_eq!(serde_json::to_value(status).unwrap(), wire);
        assert_eq!(serde_json::to_string(&status).unwrap(), exact);
    }
}

#[test]
fn chat_updated_carries_the_rest_descriptor_and_never_the_hashes() {
    let chat = Chat {
        id: id(20),
        title: "general".into(),
        password_hash: "secret-digest".into(),
        access_hash: "secret-hash".into(),
        creator_user_id: Some(id(3)),
        member_count: 2,
        created_at: when(),
        ..Chat::default()
    };
    let value = serialized(&ChatMessage::ChatUpdated { chat });
    assert_eq!(value["type"], "chat_updated");
    assert_eq!(value["chat"]["id"], json!(id(20)));
    assert_eq!(value["chat"]["chat_type"], "group");
    assert_eq!(value["chat"]["title"], "general");
    assert_eq!(value["chat"]["member_count"], 2);
    assert!(value["chat"].get("password_hash").is_none());
    assert!(value["chat"].get("access_hash").is_none());
    assert!(
        value["chat"].get("name").is_none(),
        "the deprecated spelling stays REST-only"
    );
}

#[test]
fn member_updated_reuses_the_rest_membership_shape() {
    let frame = ChatMessage::MemberUpdated {
        member: ChatMembership {
            user_id: id(3),
            username: "alice".into(),
            avatar_emoji: "🦀".into(),
            nickname: "Al".into(),
            role: "member".into(),
            status: "active".into(),
            requested_at: when(),
            joined_at: Some(when()),
        },
    };
    assert_eq!(
        serialized(&frame),
        json!({
            "type": "member_updated",
            "member": { "user_id": id(3), "username": "alice", "avatar_emoji": "🦀",
                "nickname": "Al", "role": "member", "status": "active",
                "requested_at": "2026-09-30T12:00:00Z", "joined_at": "2026-09-30T12:00:00Z" }
        })
    );
}

#[test]
fn topic_views_and_poll_skeletons_serialize_as_frozen() {
    assert_eq!(
        serialized(&ChatMessage::TopicUpdated {
            topic: TopicSummary {
                id: id(30),
                title: "release plan".into(),
                icon_emoji: "📌".into(),
                closed: false,
                pinned: true,
            },
        }),
        json!({ "type": "topic_updated", "topic": { "id": id(30), "title": "release plan",
            "icon_emoji": "📌", "closed": false, "pinned": true } })
    );
    assert_eq!(
        serialized(&ChatMessage::MessageViewsUpdated {
            views: vec![
                MessageViewCount {
                    message_id: id(40),
                    views: 7
                },
                MessageViewCount {
                    message_id: id(41),
                    views: 1200
                },
            ],
        }),
        json!({ "type": "message_views_updated", "views": [
            { "message_id": id(40), "views": 7 },
            { "message_id": id(41), "views": 1200 }
        ] })
    );
    assert_eq!(
        serialized(&ChatMessage::PollUpdated {
            message_id: id(50),
            poll: PollState {
                id: id(51),
                question: "lunch?".into(),
                closed: false,
                total_voters: 3,
                options: vec![
                    PollOption {
                        text: "yes".into(),
                        voters: 2
                    },
                    PollOption {
                        text: "no".into(),
                        voters: 1
                    },
                ],
                ..PollState::default()
            },
        }),
        json!({ "type": "poll_updated", "message_id": id(50), "poll": {
            "id": id(51), "question": "lunch?", "closed": false, "total_voters": 3,
            "options": [ { "text": "yes", "voters": 2 }, { "text": "no", "voters": 1 } ]
        } })
    );
}

#[test]
fn draft_updated_matches_the_tg008_columns() {
    assert_eq!(
        serialized(&ChatMessage::DraftUpdated {
            user_id: id(3),
            text: "unsent thought".into(),
            reply_to_message_id: Some(id(10)),
            topic_id: None,
            updated_at: when(),
        }),
        json!({ "type": "draft_updated", "user_id": id(3), "text": "unsent thought",
            "reply_to_message_id": id(10), "topic_id": null,
            "updated_at": "2026-09-30T12:00:00Z" })
    );
}
