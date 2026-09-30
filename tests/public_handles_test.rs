//! TG-206 public handles. Handle rules and the shared namespace with user logins are enforced;
//! a visitor sees only the preview fields (never the internal id, never messages) and joins
//! through the handle; discovery finds public chats by handle prefix and title.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres};

async fn public_handles_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let owner = server.register("ph-owner").await;
    let visitor = server.register("ph-visitor").await;
    let chat = server.create_group(&owner, "Rust Learners").await;
    let set = |token: String, body: serde_json::Value| {
        let server = &server;
        let chat = chat.clone();
        async move {
            server
                .call(
                    Method::PUT,
                    &format!("/api/chats/{chat}/username"),
                    &token,
                    Some(body),
                )
                .await
        }
    };

    // Rules, reserved words, and the namespace shared with user logins.
    let (status, body) = set(owner.token.clone(), json!({ "username": "ab" })).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::BAD_REQUEST, json!("too_short"))
    );
    let (status, body) = set(owner.token.clone(), json!({ "username": "support" })).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::BAD_REQUEST, json!("reserved"))
    );
    let (status, body) = set(owner.token.clone(), json!({ "username": "ph-visitor" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    server.register("phloginname").await;
    let (status, body) = set(owner.token.clone(), json!({ "username": "PhLoginName" })).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::CONFLICT, json!("taken")),
        "a user's login is taken in the shared namespace"
    );
    let (status, body) = set(owner.token.clone(), json!({ "username": "ph_owner_x" })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = set(
        visitor.token.clone(),
        json!({ "username": "rust_learners" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "only chat.info holders");
    let (status, chat_view) =
        set(owner.token.clone(), json!({ "username": "Rust_Learners" })).await;
    assert_eq!(status, StatusCode::OK, "{chat_view}");
    assert_eq!(chat_view["username"], "rust_learners", "stored lowercase");
    assert_eq!(
        chat_view["chat_type"], "supergroup",
        "a public handle upgrades a group"
    );

    // Availability: taken by this chat, case-insensitively.
    let (_, check) = server
        .get("/api/public-usernames/RUST_learners/check", &visitor.token)
        .await;
    assert_eq!(check["available"], false);
    assert_eq!(check["reason"], "taken");
    let (_, free) = server
        .get("/api/public-usernames/rust_mentors/check", &visitor.token)
        .await;
    assert_eq!(free["available"], true);

    // A second chat cannot take the same handle.
    let other = server.create_group(&owner, "Other").await;
    let (status, taken) = server
        .call(
            Method::PUT,
            &format!("/api/chats/{other}/username"),
            &owner.token,
            Some(json!({ "username": "rust_learners" })),
        )
        .await;
    assert_eq!(
        (status, taken["error"].clone()),
        (StatusCode::CONFLICT, json!("taken"))
    );

    // The visitor's preview: only the allowed fields, no id.
    let (status, preview) = server
        .get("/api/public/rust_learners", &visitor.token)
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["title"], "Rust Learners");
    assert_eq!(preview["member_count"], 1);
    assert_eq!(preview["is_member"], false);
    assert!(
        preview.get("chat_id").is_none(),
        "a visitor never learns the id: {preview}"
    );
    assert!(preview.get("access_hash").is_none());
    let (status, _) = server
        .get(&format!("/api/chats/{chat}/messages"), &visitor.token)
        .await;
    assert_ne!(status, StatusCode::OK, "no history before joining");

    // Discovery by handle prefix (with or without @) and by title.
    for q in ["rust_le", "@rust", "learners"] {
        let (status, found) = server
            .get(&format!("/api/chats/discover?q={q}"), &visitor.token)
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            found
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["title"] == "Rust Learners"),
            "q={q}: {found}"
        );
    }

    // Join through the handle; now the id is shown.
    let (status, joined) = server
        .call(
            Method::POST,
            "/api/public/rust_learners/join",
            &visitor.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    let (_, member_view) = server
        .get("/api/public/rust_learners", &visitor.token)
        .await;
    assert_eq!(member_view["is_member"], true);
    assert_eq!(member_view["chat_id"], chat.as_str());

    // Clearing the handle makes it unresolvable.
    let (status, _) = set(owner.token.clone(), json!({ "username": null })).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .get("/api/public/rust_learners", &visitor.token)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sqlite_public_handles_resolve_preview_and_join() {
    public_handles_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_public_handles_resolve_preview_and_join() {
    with_postgres("postgres_public_handles", public_handles_scenario).await;
}
