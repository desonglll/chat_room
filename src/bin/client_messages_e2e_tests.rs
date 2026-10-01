//! TG-1205: the terminal client's quote reply, silent send, reactions, scheduled messages,
//! in-chat search and invite-link joins against a real in-process server — through the TUI's own
//! socket client (`client_chat::connect`) and API client.

use std::{sync::Arc, time::Duration};

use chat_room::{build_app, config::AppConfig, state::AppState};
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::client_api::ApiClient;
use crate::client_chat::{self, ChatCommand, ChatConnection, ChatEvent, ChatMessage};

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(
        AppState::new_with_config(&AppConfig::default())
            .await
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    (base, task)
}

async fn account(base: &str, name: &str) -> (ApiClient, Uuid) {
    let session = ApiClient::new(base, None)
        .authenticate(true, name, "correct-horse-7")
        .await
        .unwrap();
    (ApiClient::new(base, Some(session.token)), session.token)
}

/// The next event matching `pick`, skipping everything else (history, typing, system lines).
async fn next<T>(
    connection: &mut ChatConnection,
    mut pick: impl FnMut(ChatEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = connection.events.recv().await.expect("chat socket closed");
            if let Some(found) = pick(event) {
                return found;
            }
        }
    })
    .await
    .expect("timed out waiting for a chat event")
}

fn message_with(content: &'static str) -> impl FnMut(ChatEvent) -> Option<ChatMessage> {
    move |event| match event {
        ChatEvent::Message(message) if message.content == content => Some(message),
        _ => None,
    }
}

fn send(content: &str, reply_to: Option<Uuid>, silent: bool, quote: Option<&str>) -> ChatCommand {
    ChatCommand::Send {
        content: content.into(),
        reply_to,
        client_message_id: Uuid::new_v4(),
        silent,
        reply_quote: quote.map(str::to_string),
    }
}

#[tokio::test]
async fn quote_silent_reactions_schedule_search_and_invites_round_trip() {
    let (base, task) = server().await;
    let (alice, alice_token) = account(&base, "tui_quote_alice").await;
    let (bob, bob_token) = account(&base, "tui_quote_bob").await;
    let group = alice.create_chat("tui ops", None).await.unwrap().id;

    // Bob joins through an invite link Alice made (pasted as a full link).
    let link: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/chats/{group}/invite-links"))
        .bearer_auth(alice_token)
        .json(&serde_json::json!({ "title": "tui" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = link["token"].as_str().expect("invite token").to_string();
    let pasted = format!("{base}/joinchat/{token}");
    let parsed = crate::client_api_messages::invite_token(&pasted).unwrap();
    let joined = bob.join_invite(&parsed).await.unwrap();
    assert_eq!(
        (joined.status.as_str(), joined.chat_id),
        ("active", Some(group))
    );
    assert_eq!(
        bob.join_invite("AAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .unwrap_err()
            .status,
        Some(reqwest::StatusCode::NOT_FOUND)
    );

    let mut a = client_chat::connect(&base, group, alice_token, None)
        .await
        .unwrap();
    let mut b = client_chat::connect(&base, group, bob_token, None)
        .await
        .unwrap();

    // Quote reply: Bob sees what Alice's answer quotes.
    a.sender
        .send(send("are we shipping on friday", None, false, None))
        .unwrap();
    let question = next(&mut b, message_with("are we shipping on friday")).await;
    a.sender
        .send(send("yes", Some(question.id), false, Some("on friday")))
        .unwrap();
    let answer = next(&mut b, message_with("yes")).await;
    assert_eq!(
        answer.extras.reply_line().unwrap(),
        "↳ tui_quote_alice: “on friday”"
    );

    // Silent send arrives marked silent.
    b.sender.send(send("good night", None, true, None)).unwrap();
    let quiet = next(&mut a, message_with("good night")).await;
    assert!(quiet.extras.silent);

    // Reactions carry the reacting user, so counts can be kept and removed.
    b.sender
        .send(ChatCommand::React {
            message_id: question.id,
            emoji: "👍".into(),
            active: true,
        })
        .unwrap();
    let mut extras = (*question.extras).clone();
    let (emoji, user, active) = next(&mut a, |event| match event {
        ChatEvent::ReactionChanged {
            emoji,
            user_id,
            active,
            ..
        } => Some((emoji, user_id, active)),
        _ => None,
    })
    .await;
    assert!(user.is_some() && active);
    extras.apply_reaction(&emoji, user, active);
    assert_eq!(extras.reaction_line().unwrap(), "👍 1");

    // Schedule, list, send now (delivered over the socket), schedule again and cancel.
    let at = chrono::Utc::now() + chrono::Duration::hours(2);
    let scheduled = alice
        .schedule_message(group, "later ping", at, true)
        .await
        .unwrap();
    assert!(scheduled.silent);
    assert_eq!(
        alice.scheduled_messages(group).await.unwrap(),
        vec![scheduled.clone()]
    );
    alice.send_scheduled_now(group, scheduled.id).await.unwrap();
    next(&mut b, message_with("later ping")).await;
    let second = alice
        .schedule_message(group, "never", at, false)
        .await
        .unwrap();
    alice.cancel_scheduled(group, second.id).await.unwrap();
    assert!(alice.scheduled_messages(group).await.unwrap().is_empty());

    // In-chat search finds the question by a word.
    let hits = bob.search_chat(group, None, "shipping").await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        (hits[0].id, hits[0].sender.as_str()),
        (question.id, "tui_quote_alice")
    );

    let _ = a.sender.send(ChatCommand::Close);
    let _ = b.sender.send(ChatCommand::Close);
    task.abort();
}
