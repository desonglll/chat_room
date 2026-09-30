//! Transport paths for the TG-007 frame skeletons: every new variant traverses
//! `AppState::broadcast` → chat channel → forwarder → socket, `draft_updated` reaches only
//! the drafting account's connections, and unknown inbound frames are ignored.

use chat_room::models::{
    Chat, ChatMembership, ChatMessage, MessageViewCount, PollState, TopicSummary,
};
use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

mod ws_frame_support;
use ws_frame_support::{collect_until, create_chat, open_chat, session_token, start_server};

#[tokio::test]
async fn draft_updated_reaches_only_the_drafting_accounts_connections() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "drafts", "dr-alice").await;
    let alice_token = session_token(&server.base, "dr-alice").await;
    let bob_token = session_token(&server.base, "dr-bob").await;
    let alice_id = ws_frame_support::user_id(&server.state, &alice_token).await;

    let (mut alice_first, _) = open_chat(&server.base, room_id, &alice_token).await;
    let (mut alice_second, _) = open_chat(&server.base, room_id, &alice_token).await;
    let (mut bob, _) = open_chat(&server.base, room_id, &bob_token).await;

    server
        .state
        .broadcast(
            room_id,
            ChatMessage::DraftUpdated {
                user_id: alice_id,
                text: "unsent thought".into(),
                reply_to_message_id: None,
                topic_id: None,
                updated_at: chrono::Utc::now(),
            },
        )
        .await;
    server
        .state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: "marker".into(),
                members: None,
                participants: None,
            },
        )
        .await;

    for socket in [&mut alice_first, &mut alice_second] {
        let (draft, _) = collect_until(socket, "draft_updated").await;
        assert_eq!(draft["text"], "unsent thought");
        assert_eq!(draft["user_id"], alice_id.to_string());
    }
    // Bob's own "joined the room" system frame can race ahead of the marker, so skip
    // system frames until the marker itself arrives.
    let mut skipped = Vec::new();
    loop {
        let (frame, mut seen) = collect_until(&mut bob, "system").await;
        skipped.append(&mut seen);
        if frame["content"] == "marker" {
            break;
        }
    }
    assert!(
        !skipped.iter().any(|kind| kind == "draft_updated"),
        "another account's draft leaked to bob: {skipped:?}"
    );
}

#[tokio::test]
async fn skeleton_frames_traverse_the_broadcast_path() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "skeletons", "sk-alice").await;
    let alice_token = session_token(&server.base, "sk-alice").await;
    let (mut alice, _) = open_chat(&server.base, room_id, &alice_token).await;

    let frames = [
        ChatMessage::ChatUpdated {
            chat: Chat {
                id: room_id,
                title: "skeletons".into(),
                ..Default::default()
            },
        },
        ChatMessage::MemberUpdated {
            member: ChatMembership {
                user_id: Uuid::new_v4(),
                username: "sk-bob".into(),
                avatar_emoji: String::new(),
                nickname: String::new(),
                role: "member".into(),
                status: "active".into(),
                requested_at: chrono::Utc::now(),
                joined_at: None,
            },
        },
        ChatMessage::TopicUpdated {
            topic: TopicSummary {
                id: Uuid::new_v4(),
                title: "topic".into(),
                icon_emoji: String::new(),
                closed: false,
                pinned: false,
                ..Default::default()
            },
        },
        ChatMessage::MessageViewsUpdated {
            views: vec![MessageViewCount {
                message_id: Uuid::new_v4(),
                views: 5,
            }],
        },
        ChatMessage::PollUpdated {
            message_id: Uuid::new_v4(),
            poll: PollState {
                id: Uuid::new_v4(),
                question: "q".into(),
                closed: false,
                total_voters: 0,
                options: Vec::new(),
                ..PollState::default()
            },
        },
    ];
    for frame in frames {
        server.state.broadcast(room_id, frame).await;
    }
    for expected in [
        "chat_updated",
        "member_updated",
        "topic_updated",
        "message_views_updated",
        "poll_updated",
    ] {
        let (frame, _) = collect_until(&mut alice, expected).await;
        assert_eq!(frame["type"], expected);
    }
}

#[tokio::test]
async fn unknown_inbound_frames_are_ignored_and_the_connection_survives() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "tolerant", "to-alice").await;
    let alice_token = session_token(&server.base, "to-alice").await;
    let (mut alice, _) = open_chat(&server.base, room_id, &alice_token).await;

    // A frame kind the server has never heard of, and server-only kinds sent by a client.
    // The rogue user id distinguishes an injected user_status from the legitimate one the
    // server broadcasts for alice's own connection.
    let rogue_id = Uuid::new_v4();
    for rogue in [
        serde_json::json!({ "type": "levitate", "height": 3 }),
        serde_json::json!({ "type": "chat_updated", "chat": {} }),
        serde_json::json!({ "type": "user_status", "user_id": rogue_id,
            "status": { "kind": "online" } }),
    ] {
        alice.send(Message::Text(rogue.to_string())).await.unwrap();
    }
    alice
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "still alive" }).to_string(),
        ))
        .await
        .unwrap();
    let mut seen = Vec::new();
    let broadcast = loop {
        let value = ws_frame_support::next_json(&mut alice).await;
        if value["type"] == "broadcast" {
            break value;
        }
        seen.push(value);
    };
    assert_eq!(broadcast["content"], "still alive");
    assert!(
        !seen.iter().any(|value| value["type"] == "chat_updated"
            || (value["type"] == "user_status" && value["user_id"] == rogue_id.to_string())),
        "a client-injected server frame was rebroadcast: {seen:?}"
    );
}
