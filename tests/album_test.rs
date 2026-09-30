//! TG-403 albums: 2-10 media messages sent as one unit, on both database adapters.
//!
//! - a send is atomic: every item appears with one shared `grouped_id`, in album order, the
//!   caption on the first item only;
//! - a failure part-way (one upload incomplete) leaves no item at all;
//! - the whole album can be recalled at once; a single item stays an ordinary recall;
//! - forwarding two or more items of an album keeps them grouped in the target.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres, Account, Server};

/// Open an upload session and send `bytes` of it (all of it unless `partial`).
async fn upload(
    server: &Server,
    chat: &str,
    account: &Account,
    name: &str,
    partial: bool,
) -> String {
    let bytes = format!("album item {name}").into_bytes();
    let (status, created) = server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/attachments/uploads"),
            &account.token,
            Some(json!({ "file_name": name, "mime_type": "image/png",
                "size_bytes": bytes.len(), "fingerprint": uuid::Uuid::new_v4().to_string() })),
        )
        .await;
    assert!(status.is_success(), "{status} {created}");
    let upload_id = created["upload_id"].as_str().unwrap().to_string();
    let sent = if partial { &bytes[..3] } else { &bytes[..] };
    let response = server
        .client
        .put(format!(
            "{}/api/attachments/uploads/{upload_id}/chunks?offset=0",
            server.base
        ))
        .bearer_auth(&account.token)
        .body(sent.to_vec())
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success(), "{}", response.status());
    upload_id
}

async fn send_album(
    server: &Server,
    chat: &str,
    account: &Account,
    body: Value,
) -> (StatusCode, Value) {
    server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/albums"),
            &account.token,
            Some(body),
        )
        .await
}

async fn history(server: &Server, chat: &str, account: &Account) -> Vec<Value> {
    let (status, page) = server
        .get(&format!("/api/chats/{chat}/messages"), &account.token)
        .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page.as_array().cloned().unwrap_or_default()
}

async fn album_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("album-alice").await;
    let bob = server.register("album-bob").await;
    let chat = server.create_group(&alice, "albums").await;
    server.join(&chat, &bob).await;

    // Atomic send, album order, caption on the first item only.
    let uploads = [
        upload(&server, &chat, &alice, "one.png", false).await,
        upload(&server, &chat, &alice, "two.png", false).await,
        upload(&server, &chat, &alice, "three.png", false).await,
    ];
    let (status, sent) = send_album(
        &server,
        &chat,
        &alice,
        json!({ "upload_ids": uploads, "caption": "trip" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{sent}");
    let grouped_id = sent["grouped_id"].as_str().unwrap().to_string();
    let items: Vec<Value> = history(&server, &chat, &bob)
        .await
        .into_iter()
        .filter(|message| message["grouped_id"] == grouped_id.as_str())
        .collect();
    assert_eq!(items.len(), 3, "every item is in history");
    let names: Vec<&str> = items
        .iter()
        .map(|item| item["attachment"]["file_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["one.png", "two.png", "three.png"], "album order");
    assert_eq!(items[0]["content"], "trip");
    assert_eq!(items[1]["content"], "");

    // A failure part-way leaves nothing behind.
    let before = history(&server, &chat, &bob).await.len();
    let broken = [
        upload(&server, &chat, &alice, "ok.png", false).await,
        upload(&server, &chat, &alice, "cut.png", true).await,
    ];
    let (status, refused) =
        send_album(&server, &chat, &alice, json!({ "upload_ids": broken })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(
        history(&server, &chat, &bob).await.len(),
        before,
        "no partial album"
    );

    // One item is not an album.
    let single = [upload(&server, &chat, &alice, "solo.png", false).await];
    let (status, _) = send_album(&server, &chat, &alice, json!({ "upload_ids": single })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Forwarding two items of the album keeps them grouped (with a new id) in the target.
    let target = server.create_group(&alice, "album-target").await;
    let forwarded_ids: Vec<&str> = items[..2]
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    let (status, forwarded) = server
        .call(
            Method::POST,
            "/api/messages/forward",
            &alice.token,
            Some(json!({ "message_ids": forwarded_ids, "target_room_ids": [target] })),
        )
        .await;
    assert!(status.is_success(), "{status} {forwarded}");
    let copies = history(&server, &target, &alice).await;
    let groups: Vec<&Value> = copies.iter().map(|copy| &copy["grouped_id"]).collect();
    assert_eq!(groups.len(), 2, "{copies:?}");
    assert!(groups[0].is_string(), "forwarded items stay an album");
    assert_eq!(groups[0], groups[1]);
    assert_ne!(
        groups[0],
        grouped_id.as_str(),
        "a fresh group in the target"
    );

    // Recalling the whole album recalls every item; bob cannot recall alice's album.
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/chats/{chat}/albums/{grouped_id}"),
            &bob.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, recalled) = server
        .call(
            Method::DELETE,
            &format!("/api/chats/{chat}/albums/{grouped_id}"),
            &alice.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{recalled}");
    assert_eq!(recalled["recalled"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn sqlite_albums_are_atomic_ordered_forwardable_and_recallable() {
    album_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_albums_are_atomic_ordered_forwardable_and_recallable() {
    with_postgres("postgres_albums", album_scenario).await;
}
