//! TG-006: `/api/chats/*` is the contract and `/api/rooms/*` is a routing-only alias onto the
//! same handlers. Anything the two paths disagree about beyond the deprecated `name` field is a
//! bug in the alias, and anything the alias stops serving breaks the frozen Vue, PySide6 and
//! ratatui clients.

use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use tokio::{net::TcpListener, task::JoinHandle};

mod support;
use support::session_token;

/// Every chat-scoped suffix, with the method used to reach it. `src/routes.rs` builds the
/// canonical tree and the alias from one list; this table is the independent check that the
/// list itself did not lose an endpoint.
const CHAT_SCOPED_GET_SUFFIXES: [&str; 6] = [
    "",
    "/discover",
    "/:id",
    "/:id/messages",
    "/:id/tasks",
    "/:id/pins",
];

async fn start_server() -> (String, Arc<AppState>, JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state, task)
}

async fn get(base: &str, path: &str, token: &str) -> (reqwest::StatusCode, serde_json::Value) {
    let response = reqwest::Client::new()
        .get(format!("{base}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    (
        status,
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
    )
}

/// Strips the fields that legitimately differ between the two dialects so the rest can be
/// compared byte for byte.
fn without_dialect_fields(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Array(items) => {
            let stripped = items.drain(..).map(without_dialect_fields).collect();
            serde_json::Value::Array(stripped)
        }
        serde_json::Value::Object(fields) => {
            fields.remove("name");
            let stripped = fields
                .iter()
                .map(|(key, item)| (key.clone(), without_dialect_fields(item.clone())))
                .collect();
            serde_json::Value::Object(stripped)
        }
        _ => value,
    }
}

#[tokio::test]
async fn the_deprecated_alias_reaches_the_same_handlers_with_the_pre_rename_field_names() {
    let (base, _state, _task) = start_server().await;
    let token = session_token(&base, "alias-owner").await;

    // A pre-rename client posts `name`; a post-rename client posts `title`. Both spellings are
    // accepted on both paths, so a client never has to know which era the server is from.
    let created: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "name": "alias-chat", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["name"], "alias-chat", "the alias answers in `name`");
    assert_eq!(created["title"], "alias-chat", "and in `title`");
    let chat_id = created["id"].as_str().unwrap().to_string();

    let modern: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "canonical-chat", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(modern["title"], "canonical-chat");
    assert!(
        modern.get("name").is_none(),
        "the canonical path must not carry the deprecated field: {modern}"
    );

    for suffix in CHAT_SCOPED_GET_SUFFIXES {
        let suffix = suffix.replace(":id", &chat_id);
        let (chat_status, chat_body) = get(&base, &format!("/api/chats{suffix}"), &token).await;
        let (alias_status, alias_body) = get(&base, &format!("/api/rooms{suffix}"), &token).await;
        assert_eq!(
            chat_status, alias_status,
            "/api/chats{suffix} and /api/rooms{suffix} disagree on status"
        );
        assert_ne!(
            chat_status,
            reqwest::StatusCode::NOT_FOUND,
            "/api/chats{suffix} is not routed at all"
        );
        assert_eq!(
            without_dialect_fields(chat_body),
            without_dialect_fields(alias_body),
            "/api/chats{suffix} and /api/rooms{suffix} returned different bodies"
        );
    }

    // The alias carries the write verbs too, not only the reads.
    let renamed = reqwest::Client::new()
        .patch(format!("{base}/api/rooms/{chat_id}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "name": "alias-renamed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), 200);
    let renamed: serde_json::Value = renamed.json().await.unwrap();
    assert_eq!(renamed["name"], "alias-renamed");
    assert_eq!(renamed["title"], "alias-renamed");

    let patched_canonically = reqwest::Client::new()
        .patch(format!("{base}/api/chats/{chat_id}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "canonically-renamed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(patched_canonically.status(), 200);
    let patched_canonically: serde_json::Value = patched_canonically.json().await.unwrap();
    assert_eq!(patched_canonically["title"], "canonically-renamed");
    assert!(patched_canonically.get("name").is_none());
}

#[tokio::test]
async fn a_new_chat_is_a_group_with_the_documented_defaults() {
    let (base, _state, _task) = start_server().await;
    let token = session_token(&base, "defaults-owner").await;
    let created: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "defaults-chat" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(created["chat_type"], "group");
    assert_eq!(created["username"], serde_json::Value::Null);
    assert_eq!(created["is_forum"], false);
    assert_eq!(created["linked_chat_id"], serde_json::Value::Null);
    assert_eq!(created["slow_mode_seconds"], 0);
    assert_eq!(created["auto_delete_seconds"], 0);
    assert_eq!(created["signatures_enabled"], false);
    assert_eq!(created["history_visible_to_new_members"], true);
    assert_eq!(
        created["member_count"], 1,
        "the creator is the first member"
    );
    assert!(
        created.get("access_hash").is_none(),
        "access_hash is never serialised: {created}"
    );
}

#[tokio::test]
async fn the_conversation_list_keeps_both_field_names_because_it_has_no_alias() {
    let (base, _state, _task) = start_server().await;
    let token = session_token(&base, "conversation-owner").await;
    reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "title": "conversation-chat" }))
        .send()
        .await
        .unwrap();

    let (status, conversations) = get(&base, "/api/conversations", &token).await;
    assert_eq!(status, 200);
    let group = &conversations[0]["group"];
    assert_eq!(group["title"], "conversation-chat");
    assert_eq!(
        group["name"], "conversation-chat",
        "/api/conversations has no deprecated alias of its own, so it serves both spellings \
         until the pre-rename clients are deleted in M6: {conversations}"
    );
    assert_eq!(group["chat_type"], "group");
}

#[tokio::test]
async fn the_openapi_document_marks_every_alias_operation_deprecated() {
    let spec = chat_room::chats::compat::openapi_with_deprecated_chat_alias();
    let mut aliased = 0;
    for (path, item) in &spec.paths.paths {
        if let Some(suffix) = path.strip_prefix("/api/rooms") {
            aliased += 1;
            assert!(
                spec.paths
                    .paths
                    .contains_key(&format!("/api/chats{suffix}")),
                "{path} has no canonical counterpart"
            );
            for operation in item.operations.values() {
                assert!(
                    matches!(
                        operation.deprecated,
                        Some(utoipa::openapi::Deprecated::True)
                    ),
                    "an operation on {path} is not marked deprecated"
                );
            }
        } else if let Some(suffix) = path.strip_prefix("/api/chats") {
            for operation in item.operations.values() {
                assert!(
                    !matches!(
                        operation.deprecated,
                        Some(utoipa::openapi::Deprecated::True)
                    ),
                    "an operation on /api/chats{suffix} must not be deprecated"
                );
            }
        }
    }
    assert!(
        aliased >= 5,
        "expected the annotated chat operations to be mirrored onto the alias, got {aliased}"
    );
}
