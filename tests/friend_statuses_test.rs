//! TG-903: `GET /api/friends/statuses` gives the contacts list each friend's presence under the
//! TG-505 `last_seen` rules — exact (online / offline at a time) only when both sides admit
//! each other, the obscured tier otherwise; strangers never appear.

mod privacy_support;

use privacy_support::{start_server, Account, TestServer};
use reqwest::StatusCode;
use serde_json::Value;

async fn statuses(server: &TestServer, viewer: &Account) -> Vec<Value> {
    let response = server
        .client
        .get(server.url("/api/friends/statuses"))
        .bearer_auth(&viewer.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

fn status_of(entries: &[Value], account: &Account) -> Value {
    entries
        .iter()
        .find(|entry| entry["user_id"] == account.id.to_string())
        .map(|entry| entry["status"].clone())
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn friends_statuses_follow_last_seen_privacy() {
    let server = start_server().await;
    let alice = server.account("fs-alice").await;
    let bob = server.account("fs-bob").await;
    let stranger = server.account("fs-stranger").await;
    server.befriend(&alice, &bob).await;

    // Bob is connected to a chat: Alice sees him online; strangers are not listed.
    let chat = server.create_group(&bob, "fs-chat").await;
    let (socket, _) = server.open_chat(chat, &bob).await;
    let entries = statuses(&server, &alice).await;
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(status_of(&entries, &bob)["kind"], "online", "{entries:?}");
    assert!(status_of(&entries, &stranger).is_null());

    // Bob hides his last seen from everybody: Alice gets only the obscured tier.
    let (status, _) = server.put_rule(&bob, "last_seen", "nobody", &[], &[]).await;
    assert_eq!(status, StatusCode::OK);
    let hidden = status_of(&statuses(&server, &alice).await, &bob);
    assert!(
        ["recently", "within_week", "within_month", "long_ago"]
            .contains(&hidden["kind"].as_str().unwrap()),
        "{hidden}"
    );

    // Reciprocity: Bob admits Alice again, but Alice hides hers — she loses his exact status.
    server
        .put_rule(&bob, "last_seen", "everybody", &[], &[])
        .await;
    server
        .put_rule(&alice, "last_seen", "nobody", &[], &[])
        .await;
    let reciprocal = status_of(&statuses(&server, &alice).await, &bob);
    assert_ne!(reciprocal["kind"], "online", "{reciprocal}");
    server
        .put_rule(&alice, "last_seen", "everybody", &[], &[])
        .await;

    // Offline after disconnecting: an exact last-seen time.
    server.close_and_settle(chat, &bob, socket).await;
    let offline = status_of(&statuses(&server, &alice).await, &bob);
    assert_eq!(offline["kind"], "offline", "{offline}");
    assert!(offline["last_seen"].is_string());

    let response = server
        .client
        .get(server.url("/api/friends/statuses"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
