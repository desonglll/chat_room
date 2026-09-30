//! TG-406 authorization: anonymous polls never reveal a voter — not through REST, history,
//! the voter list, or any WebSocket frame — and nothing about a poll reaches a non-member.

mod poll_support;

use std::time::Duration;

use chat_room::messages::polls::POLL_BROADCAST_WINDOW;
use poll_support::*;
use reqwest::{Method, StatusCode};
use serde_json::json;

fn voters_url(base: &str, poll: impl std::fmt::Display, option: u32) -> String {
    format!("{base}/api/polls/{poll}/voters?option={option}")
}

#[tokio::test]
async fn anonymous_polls_never_reveal_voters_on_any_channel() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "anon-alice").await;
    let bob = register(base, "anon-bob").await;
    let chat = create_chat(base, &alice, "anon").await;
    join(base, &bob, chat).await;
    let poll = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Secret?", "options": ["a", "b"] }),
    )
    .await;
    let mut alice_socket = open_socket(base, chat, &alice).await;
    let bob_id = bob.id.to_string();

    // Bob votes, changes his mind, votes again: several aggregated frames.
    vote(base, &bob, poll, &[0]).await;
    tokio::time::sleep(POLL_BROADCAST_WINDOW * 2).await;
    vote(base, &bob, poll, &[1]).await;
    let frames = drain_type(&mut alice_socket, "poll_updated", POLL_BROADCAST_WINDOW * 4).await;
    assert!(!frames.is_empty(), "the author still sees live counts");
    for frame in &frames {
        assert!(
            !frame.contains(&bob_id),
            "poll_updated leaked a voter: {frame}"
        );
    }

    // A fresh connection's history replay, the REST history and the poll read: no voter id.
    let mut replay = open_socket(base, chat, &alice).await;
    drop(drain_type(&mut replay, "broadcast", Duration::from_millis(100)).await);
    let (_, state) = get_poll(base, &alice, poll).await;
    assert_eq!(state["total_voters"], 1);
    assert!(!state.to_string().contains(&bob_id));
    let message = history(base, &alice, chat)
        .await
        .into_iter()
        .find(|message| message["id"] == poll.to_string())
        .unwrap();
    assert!(!message.to_string().contains(&bob_id));

    // The voter list is refused to everyone, the author and the voter included.
    for account in [&alice, &bob] {
        let (status, body) =
            call(Method::GET, voters_url(base, poll, 1), &account.token, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!body.to_string().contains(&bob_id));
    }
}

#[tokio::test]
async fn history_replay_frames_of_an_anonymous_poll_hold_no_voter() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "anon-replay-alice").await;
    let bob = register(base, "anon-replay-bob").await;
    let chat = create_chat(base, &alice, "anon-replay").await;
    join(base, &bob, chat).await;
    let poll = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Q", "options": ["a", "b"] }),
    )
    .await;
    vote(base, &bob, poll, &[1]).await;

    // Capture the raw replay: every `broadcast` frame before `history_complete`.
    let url = format!("{}/ws/{chat}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    use futures_util::{SinkExt, StreamExt};
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            json!({ "type": "join", "token": alice.token }).to_string(),
        ))
        .await
        .unwrap();
    let mut replayed = Vec::new();
    while let Some(Ok(frame)) = socket.next().await {
        let text = frame.into_text().unwrap().to_string();
        if text.contains("\"history_complete\"") {
            break;
        }
        if text.contains("\"broadcast\"") {
            replayed.push(text);
        }
    }
    let poll_frame = replayed
        .iter()
        .find(|frame| frame.contains(&poll.to_string()))
        .expect("the poll is replayed");
    assert!(poll_frame.contains("\"total_voters\":1"));
    assert!(!poll_frame.contains(&bob.id.to_string()));
}

#[tokio::test]
async fn public_polls_list_voters_to_members_only() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "public-alice").await;
    let bob = register(base, "public-bob").await;
    let outsider = register(base, "public-outsider").await;
    let chat = create_chat(base, &alice, "public").await;
    join(base, &bob, chat).await;
    let poll = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Who?", "options": ["me", "you"], "public_voters": true }),
    )
    .await;
    vote(base, &bob, poll, &[0]).await;
    vote(base, &alice, poll, &[1]).await;

    let (status, page) = call(Method::GET, voters_url(base, poll, 0), &alice.token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 1);
    assert_eq!(page["voters"][0]["user_id"], bob.id.to_string());
    assert_eq!(page["voters"][0]["username"], "public-bob");
    assert_eq!(
        call(Method::GET, voters_url(base, poll, 2), &alice.token, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    // An outsider learns nothing, not even that the poll exists.
    for (method, url, body) in [
        (Method::GET, voters_url(base, poll, 0), None),
        (Method::GET, format!("{base}/api/polls/{poll}"), None),
        (
            Method::POST,
            format!("{base}/api/polls/{poll}/votes"),
            Some(json!({ "options": [0] })),
        ),
        (
            Method::DELETE,
            format!("{base}/api/polls/{poll}/votes"),
            None,
        ),
        (Method::POST, format!("{base}/api/polls/{poll}/close"), None),
    ] {
        let (status, body) = call(method, url, &outsider.token, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(!body.to_string().contains("Who?"));
    }

    // Leaving the chat revokes read access at read time.
    let (status, _) = call(
        Method::DELETE,
        format!("{base}/api/chats/{chat}/members/me"),
        &bob.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(get_poll(base, &bob, poll).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        call(Method::GET, voters_url(base, poll, 0), &bob.token, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn polls_need_a_session() {
    let server = serve_memory().await;
    let (status, _) = call(
        Method::GET,
        format!("{}/api/polls/{}", server.base, uuid::Uuid::new_v4()),
        "not-a-token",
        None,
    )
    .await;
    assert!(status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST);
}
