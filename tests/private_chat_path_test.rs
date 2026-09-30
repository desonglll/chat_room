//! TG-208: a `private` chat's messages flow through exactly the same code path as a group's.
//!
//! Every assertion here is made pairwise — the same request against a private chat and
//! against an approval-only group — so a private-only branch that reappears in the read path,
//! the write path or the authorization decision shows up as the two answers diverging.
//! The scenario runs on SQLite always and on PostgreSQL when `TEST_POSTGRES_ADMIN_URL` is set.

use std::sync::Arc;

use chat_room::{config::AppConfig, state::AppState};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};

#[path = "integration/postgres_database.rs"]
mod postgres_database;
mod private_chat_support;
#[path = "service_skip/mod.rs"]
mod service_skip;

use private_chat_support::{befriend, get_json, join, register, send_message, serve};

async fn unified_path_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let base = server.base.as_str();
    let client = Client::new();
    let alice = register(&client, base, "tg208-alice", "Alice A").await;
    let bob = register(&client, base, "tg208-bob", "Bob B").await;
    let outsider = register(&client, base, "tg208-outsider", "Outsider").await;
    befriend(&client, base, &alice, &bob).await;

    let private: Value = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&alice.token)
        .json(&json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let private_id = private["room_id"].as_str().unwrap().to_string();
    assert_eq!(private["chat_type"], "private");
    assert_eq!(
        private["kind"], "direct",
        "frozen clients' spelling is unchanged"
    );

    let group = client
        .post(format!("{base}/api/chats"))
        .bearer_auth(&alice.token)
        .json(&json!({ "title": "tg208-group", "join_policy": "approval" }))
        .send()
        .await
        .unwrap();
    assert_eq!(group.status(), StatusCode::CREATED);
    let group: Value = group.json().await.unwrap();
    let group_id = group["id"].as_str().unwrap().to_string();

    // Write path: the same WebSocket send, persisted into the same `messages` table.
    let _private_socket = send_message(base, &private_id, &alice.token, "private hello").await;
    let _group_socket = send_message(base, &group_id, &alice.token, "group hello").await;

    // Read path, participant: the same history endpoint returns both.
    for (chat_id, content) in [(&private_id, "private hello"), (&group_id, "group hello")] {
        let (status, history) = get_json(
            &client,
            format!("{base}/api/chats/{chat_id}/messages"),
            &alice.token,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            history
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["content"] == content),
            "{content} is readable through /api/chats/:id/messages"
        );
    }
    let (status, bob_history) = get_json(
        &client,
        format!("{base}/api/chats/{private_id}/messages"),
        &bob.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bob_history[0]["content"], "private hello");

    // Read path, non-participant: denied identically on both chat types.
    let mut history_denials = Vec::new();
    let mut socket_denials = Vec::new();
    for chat_id in [&private_id, &group_id] {
        let (status, _) = get_json(
            &client,
            format!("{base}/api/chats/{chat_id}/messages"),
            &outsider.token,
        )
        .await;
        history_denials.push(status);
        let (mut socket, reply) = join(base, chat_id, &outsider.token).await;
        assert_eq!(reply["type"], "auth_fail");
        // The server closes a refused socket itself; wait for that rather than dropping it.
        while let Some(Ok(_)) = socket.next().await {}
        socket_denials.push(reply["reason"].clone());
    }
    assert_eq!(
        history_denials,
        [StatusCode::FORBIDDEN, StatusCode::FORBIDDEN]
    );
    assert_eq!(socket_denials[0], socket_denials[1]);
    assert_eq!(socket_denials[0], "membership required");

    // Chat list: the canonical contract lists private chats like any other, presented as
    // the peer; the deprecated alias keeps hiding them from the frozen clients.
    let (_, alice_chats) = get_json(&client, format!("{base}/api/chats"), &alice.token).await;
    let alice_chats = alice_chats.as_array().unwrap();
    let listed_private = alice_chats
        .iter()
        .find(|chat| chat["id"] == private_id.as_str())
        .expect("private chat is listed on /api/chats");
    assert_eq!(listed_private["chat_type"], "private");
    assert_eq!(listed_private["title"], "Bob B");
    assert_eq!(listed_private["avatar_emoji"], "🦊");
    assert_eq!(listed_private["membership_status"], "active");
    assert_eq!(listed_private["unread_count"], 0);
    assert!(alice_chats
        .iter()
        .any(|chat| chat["id"] == group_id.as_str() && chat["chat_type"] == "group"));
    let (_, bob_chats) = get_json(&client, format!("{base}/api/chats"), &bob.token).await;
    let bob_private = bob_chats
        .as_array()
        .unwrap()
        .iter()
        .find(|chat| chat["id"] == private_id.as_str())
        .expect("private chat is listed for the peer too");
    assert_eq!(bob_private["title"], "Alice A");
    assert_eq!(bob_private["unread_count"], 1);
    let (_, outsider_chats) = get_json(&client, format!("{base}/api/chats"), &outsider.token).await;
    assert!(outsider_chats
        .as_array()
        .unwrap()
        .iter()
        .all(|chat| chat["id"] != private_id.as_str()));

    let (_, filtered) = get_json(
        &client,
        format!("{base}/api/chats?title=Bob%20B"),
        &alice.token,
    )
    .await;
    assert_eq!(filtered.as_array().unwrap().len(), 1);
    assert_eq!(filtered[0]["id"], private_id.as_str());

    let (_, legacy) = get_json(&client, format!("{base}/api/rooms"), &alice.token).await;
    let legacy = legacy.as_array().unwrap();
    assert!(legacy.iter().all(|chat| chat["id"] != private_id.as_str()));
    assert!(legacy.iter().any(|chat| chat["id"] == group_id.as_str()));

    let (_, discover) = get_json(
        &client,
        format!("{base}/api/chats/discover"),
        &outsider.token,
    )
    .await;
    assert!(discover
        .as_array()
        .unwrap()
        .iter()
        .all(|chat| chat["id"] != private_id.as_str()));

    // Detail: identical presentation on the detail endpoint, invisible to anyone else.
    let (status, detail) = get_json(
        &client,
        format!("{base}/api/chats/{private_id}"),
        &alice.token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["title"], listed_private["title"]);
    assert_eq!(detail["chat_type"], "private");
    let (status, _) = get_json(
        &client,
        format!("{base}/api/chats/{private_id}"),
        &outsider.token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Provisioning: both participants hold ordinary active memberships (the listing above
    // shows each of them the chat as `active`), while roster management — an admin surface
    // gated by `members.review` — stays closed on a private chat, as before TG-208.
    assert_eq!(bob_private["membership_status"], "active");
    let (status, _) = get_json(
        &client,
        format!("{base}/api/chats/{private_id}/members"),
        &alice.token,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Reopening is idempotent from either side.
    let reopened: Value = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&bob.token)
        .json(&json!({ "user_id": alice.id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(reopened["room_id"], private_id.as_str());

    // `/api/conversations` carries the same chat_type for both.
    let (_, conversations) =
        get_json(&client, format!("{base}/api/conversations"), &alice.token).await;
    let types: Vec<(&str, &str)> = conversations
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["chat_type"].as_str().unwrap(),
                item["kind"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(types.contains(&("private", "direct")));
    assert!(types.contains(&("group", "group")));
}

#[tokio::test]
async fn private_chat_shares_group_message_path_on_sqlite() {
    unified_path_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn private_chat_shares_group_message_path_on_postgres() {
    let Some((admin_url, admin_pool)) = postgres_database::connect_postgres_admin(
        "private_chat_shares_group_message_path_on_postgres",
    )
    .await
    else {
        return;
    };
    let (db_name, test_url) =
        postgres_database::create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&test_url, &AppConfig::default())
            .await
            .expect("open scratch PostgreSQL database"),
    );
    unified_path_scenario(state.clone()).await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    postgres_database::drop_scratch_database(&admin_pool, &db_name).await;
}
