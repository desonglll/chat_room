//! TG-901: pinning and unpinning broadcast `pins_changed` to the chat, so every open client
//! refreshes its pinned bar.

mod privacy_support;

use privacy_support::{next_json, start_server};
use reqwest::StatusCode;

#[tokio::test]
async fn pin_and_unpin_reach_open_clients() {
    let server = start_server().await;
    let owner = server.account("pc-owner").await;
    let member = server.account("pc-member").await;
    let chat = server.create_group(&owner, "pc-group").await;
    server.join(chat, &member).await;
    let message = server.send_message(chat, &owner, "pin me").await;
    let (mut socket, _) = server.open_chat(chat, &member).await;

    for (method, status, pinned) in [
        (reqwest::Method::POST, StatusCode::CREATED, true),
        (reqwest::Method::DELETE, StatusCode::NO_CONTENT, false),
    ] {
        let response = server
            .client
            .request(
                method,
                server.url(&format!("/api/chats/{chat}/pins/{message}")),
            )
            .bearer_auth(&owner.token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let frame = loop {
            let frame = next_json(&mut socket).await;
            if frame["type"] == "pins_changed" {
                break frame;
            }
        };
        assert_eq!(frame["message_id"], message.to_string());
        assert_eq!(frame["pinned"], pinned);
    }

    // A refused unpin (already unpinned) broadcasts nothing and says 404.
    let response = server
        .client
        .delete(server.url(&format!("/api/chats/{chat}/pins/{message}")))
        .bearer_auth(&owner.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
