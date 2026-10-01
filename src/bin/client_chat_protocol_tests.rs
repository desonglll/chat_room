//! Tests of `client_chat_protocol` (split out to keep that file under the size gate).
use super::*;

#[test]
fn decodes_current_broadcast_contract_without_terminal_controls() {
    let message_id = Uuid::new_v4();
    let message = decode_server_message(
        &serde_json::json!({
            "type": "broadcast",
            "message_id": message_id,
            "client_message_id": Uuid::new_v4(),
            "sender_id": null,
            "sender": "alice\u{1b}",
            "sender_avatar": "",
            "content": "hello\nworld",
            "attachment": null,
            "reply_to": null,
            "recalled_at": null,
            "edited_at": null,
            "timestamp": "2026-08-31T00:00:00Z",
            "favorite_id": null,
            "forwarded_from": null,
            "reactions": []
        })
        .to_string(),
    )
    .unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    emit_server_event(&sender, message);
    let ChatEvent::Message(message) = receiver.try_recv().unwrap() else {
        panic!("expected chat message");
    };
    assert_eq!(message.sender, "alice");
    assert_eq!(message.content, "hello\nworld");
}

#[test]
fn unknown_and_extended_frames_are_tolerated_without_events() {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    for frame in [
        // TG-007 extension frames this frozen client does not render.
        serde_json::json!({ "type": "user_status", "user_id": Uuid::new_v4(),
            "status": { "kind": "online" } }),
        serde_json::json!({ "type": "chat_updated", "chat": { "id": Uuid::new_v4() } }),
        // A frame kind that does not exist at all.
        serde_json::json!({ "type": "not_a_real_frame", "payload": 1 }),
    ] {
        let message = decode_server_message(&frame.to_string())
            .expect("unknown frame kinds must decode to the catch-all, not error");
        assert!(matches!(message, ServerMessage::Unknown));
        emit_server_event(&sender, message);
    }
    // The extended typing frame still decodes as typing; extra fields are ignored.
    let message = decode_server_message(
        &serde_json::json!({ "type": "typing", "content": "draft",
            "action": "recording_voice", "username": "alice" })
        .to_string(),
    )
    .unwrap();
    emit_server_event(&sender, message);
    let ChatEvent::Typing(Some(name)) = receiver.try_recv().unwrap() else {
        panic!("expected a typing event");
    };
    assert_eq!(name, "alice is recording a voice message");
    for (frame, expected) in [
        (
            serde_json::json!({ "type": "typing", "content": "x", "username": "bo" }),
            Some("bo is typing"),
        ),
        (
            serde_json::json!({ "type": "typing", "content": "", "username": "bo" }),
            None,
        ),
        (
            serde_json::json!({ "type": "typing", "content": "", "action": "typing", "username": "bo" }),
            None,
        ),
        (
            serde_json::json!({ "type": "typing", "content": "", "action": "cancel", "username": "bo" }),
            None,
        ),
        (
            serde_json::json!({ "type": "typing", "content": "", "action": "choosing_sticker",
            "username": "bo" }),
            Some("bo is choosing a sticker"),
        ),
    ] {
        emit_server_event(&sender, decode_server_message(&frame.to_string()).unwrap());
        let ChatEvent::Typing(line) = receiver.try_recv().unwrap() else {
            panic!("expected a typing event");
        };
        assert_eq!(line.as_deref(), expected, "{frame}");
    }
    assert!(
        receiver.try_recv().is_err(),
        "unknown frames must emit nothing"
    );
}

#[test]
fn send_commands_have_idempotency_identity() {
    let client_message_id = Uuid::new_v4();
    let frame = command_frame(ChatCommand::Send {
        content: "hello".into(),
        reply_to: None,
        client_message_id,
        silent: false,
        reply_quote: Some("ignored without reply_to".into()),
    });
    assert_eq!(frame["type"], "message");
    assert_eq!(frame["client_message_id"], client_message_id.to_string());
    assert!(frame.get("silent").is_none() && frame.get("reply_quote").is_none());
    let reply_to = Uuid::new_v4();
    let frame = command_frame(ChatCommand::Send {
        content: "yes".into(),
        reply_to: Some(reply_to),
        client_message_id,
        silent: true,
        reply_quote: Some("part".into()),
    });
    assert_eq!(frame["silent"], true);
    assert_eq!(frame["reply_quote"]["text"], "part");
    assert_eq!(frame["reply_to"], reply_to.to_string());
}

#[test]
fn broadcast_frames_carry_reply_reactions_and_silent() {
    let (reply_id, voter) = (Uuid::new_v4(), Uuid::new_v4());
    let frame = serde_json::json!({
        "type": "broadcast", "message_id": Uuid::new_v4(), "sender": "a", "content": "ok",
        "timestamp": "2026-10-01T00:00:00Z", "silent": true,
        "reply_to": { "message_id": reply_id, "sender": "b", "content": "question",
            "attachment_file_name": null, "recalled": false, "quote": { "text": "quest", "offset": 0 } },
        "reactions": [{ "emoji": "👍", "user_ids": [voter] }]
    });
    let (sender, mut receiver) = mpsc::unbounded_channel();
    emit_server_event(&sender, decode_server_message(&frame.to_string()).unwrap());
    let ChatEvent::Message(message) = receiver.try_recv().unwrap() else {
        panic!("expected chat message");
    };
    assert!(message.extras.silent);
    assert_eq!(
        message.extras.reply_to.as_ref().unwrap().message_id,
        reply_id
    );
    assert_eq!(message.extras.reply_line().unwrap(), "↳ b: “quest”");
    assert_eq!(message.extras.reaction_line().unwrap(), "👍 1");
    let changed = serde_json::json!({ "type": "reaction_changed", "message_id": Uuid::new_v4(),
        "emoji": "👍", "user_id": voter, "active": false });
    emit_server_event(
        &sender,
        decode_server_message(&changed.to_string()).unwrap(),
    );
    let ChatEvent::ReactionChanged {
        user_id, active, ..
    } = receiver.try_recv().unwrap()
    else {
        panic!("expected reaction change");
    };
    assert_eq!((user_id, active), (Some(voter), false));
}

#[test]
fn broadcast_frames_carry_poll_and_media_fields() {
    let frame = serde_json::json!({
        "type": "broadcast", "message_id": Uuid::new_v4(), "sender": "a", "content": "",
        "timestamp": "2026-10-01T00:00:00Z", "media_kind": "voice",
        "voice": { "duration_ms": 2000, "waveform": [], "listened": false },
        "poll": { "id": Uuid::new_v4(), "question": "Q?", "closed": false, "total_voters": 0,
                  "options": [{ "text": "A", "voters": 0 }] }
    });
    let ServerMessage::Broadcast { media, .. } = decode_server_message(&frame.to_string()).unwrap()
    else {
        panic!("not a broadcast");
    };
    assert_eq!(media.voice.unwrap().duration_ms, 2000);
    assert_eq!(media.poll.unwrap().question, "Q?");
    let update = serde_json::json!({ "type": "poll_updated", "message_id": Uuid::new_v4(),
        "poll": { "id": Uuid::new_v4(), "question": "Q?", "options": [] } });
    assert!(matches!(
        decode_server_message(&update.to_string()).unwrap(),
        ServerMessage::PollUpdated { .. }
    ));
}
