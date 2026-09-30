//! TG-410: contact cards and translation.
//!
//! - a contact card is a normal message (broadcast, in history) carrying a snapshot of the
//!   shared account, sent only by members allowed to send;
//! - without an AI provider translation reports itself unavailable and the endpoint answers
//!   503 (clients hide the entry); it never touches the original message.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{next_frame, serve, with_postgres};

async fn contacts_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("ct-alice").await;
    let bob = server.register("ct-bob").await;
    let carol = server.register("ct-carol").await;
    let outsider = server.register("ct-outsider").await;
    let chat = server.create_group(&alice, "ct-group").await;
    server.join(&chat, &bob).await;
    let mut bob_socket = server.socket(&chat, &bob).await;
    let path = format!("/api/chats/{chat}/contact-messages");

    let (status, sent) = server
        .call(
            Method::POST,
            &path,
            &alice.token,
            Some(json!({ "user_id": carol.id })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{sent}");
    assert_eq!(sent["media_kind"], "contact");
    assert_eq!(sent["contact"]["username"], "ct-carol");
    assert_eq!(sent["contact"]["user_id"], carol.id.as_str());

    let frame = next_frame(&mut bob_socket, "broadcast").await;
    assert_eq!(frame["contact"]["username"], "ct-carol", "delivered live");
    let (_, history) = server
        .get(&format!("/api/chats/{chat}/messages"), &bob.token)
        .await;
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["contact"]["username"] == "ct-carol"),
        "and in history"
    );

    // Non-members cannot send one; an unknown account is 404.
    let (status, _) = server
        .call(
            Method::POST,
            &path,
            &outsider.token,
            Some(json!({ "user_id": carol.id })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = server
        .call(
            Method::POST,
            &path,
            &alice.token,
            Some(json!({ "user_id": uuid::Uuid::new_v4() })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // No AI provider here: translation is unavailable and refuses (503), never a crash.
    let (_, availability) = server.get("/api/translation", &bob.token).await;
    assert_eq!(availability["available"], false);
    let message_id = sent["id"].as_str().unwrap();
    let (status, _) = server
        .call(
            Method::POST,
            &format!("/api/messages/{message_id}/translate"),
            &bob.token,
            Some(json!({ "target_language": "English" })),
        )
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn sqlite_contact_cards_and_translation_availability() {
    contacts_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_contact_cards_and_translation_availability() {
    with_postgres("postgres_contact_cards", contacts_scenario).await;
}
