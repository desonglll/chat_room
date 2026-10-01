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
    assert_eq!(name, "alice");
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
    });
    assert_eq!(frame["type"], "message");
    assert_eq!(frame["client_message_id"], client_message_id.to_string());
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
