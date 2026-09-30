//! TG-502: the archive's unarchive-on-new-message rule (`20270201000009`), on both adapters,
//! on a fresh schema and on one upgraded from the pre-TG-004 baseline.
//!
//! The rule (Telegram): a new message pulls a chat out of a member's archive unless that member
//! has the chat muted when the message arrives, or sent the message. It lives in a database
//! trigger, so the matrix drives a real server end to end: preferences over
//! `PATCH /api/conversations/{id}/preferences`, the message over the chat WebSocket, the result
//! read back from `GET /api/conversations`.

mod migration_support;

use std::{sync::Arc, time::Duration};

use chat_room::{build_app, config::AppConfig, state::AppState};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use migration_support::{
    chat_schema::{seed_postgres_pre_rename, seed_sqlite_pre_rename},
    create_postgres_scratch, drop_postgres_scratch, migrations_path, migrations_through,
    postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool, sqlite_scratch_path,
    PRE_TG_004_VERSION,
};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use sqlx::migrate::Migrator;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

struct Account {
    name: &'static str,
    token: String,
}

async fn register(client: &Client, base: &str, prefix: &str, name: &'static str) -> Account {
    let value: Value = client
        .post(format!("{base}/api/users/register"))
        .json(&json!({ "username": format!("{prefix}-{name}"), "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        name,
        token: value["token"].as_str().unwrap().to_string(),
    }
}

async fn create_chat(client: &Client, base: &str, token: &str, name: &str) -> String {
    let value: Value = client
        .post(format!("{base}/api/chats"))
        .bearer_auth(token)
        .json(&json!({ "name": name, "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    value["id"].as_str().unwrap().to_string()
}

async fn join(client: &Client, base: &str, token: &str, room_id: &str) {
    let status = client
        .post(format!("{base}/api/chats/{room_id}/join-requests"))
        .bearer_auth(token)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::OK);
}

async fn set_preferences(client: &Client, base: &str, token: &str, room_id: &str, body: Value) {
    let status = client
        .patch(format!("{base}/api/conversations/{room_id}/preferences"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::OK);
}

async fn conversation(client: &Client, base: &str, token: &str, room_id: &str) -> Value {
    let rows: Vec<Value> = client
        .get(format!("{base}/api/conversations"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    rows.into_iter()
        .find(|row| row["room_id"] == room_id)
        .expect("the member sees the conversation")
}

async fn is_archived(client: &Client, base: &str, account: &Account, room_id: &str) -> bool {
    conversation(client, base, &account.token, room_id).await["preferences"]["is_archived"]
        .as_bool()
        .unwrap()
}

/// Sends over the chat WebSocket (the production send path) and waits until the server has
/// stored it, observed through the sender's own conversation preview.
async fn send_message(client: &Client, base: &str, sender: &Account, room_id: &str, text: &str) {
    let (mut socket, _) = connect_async(format!(
        "{}/ws/{room_id}",
        base.replacen("http://", "ws://", 1)
    ))
    .await
    .unwrap();
    socket
        .send(Message::Text(
            json!({ "type": "join", "token": sender.token }).to_string(),
        ))
        .await
        .unwrap();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(text) = frame else { continue };
        if serde_json::from_str::<Value>(&text).unwrap()["type"] == "auth_ok" {
            break;
        }
    }
    socket
        .send(Message::Text(
            json!({ "type": "message", "content": text }).to_string(),
        ))
        .await
        .unwrap();
    for _ in 0..60 {
        let row = conversation(client, base, &sender.token, room_id).await;
        if row["last_message"]["content"] == text {
            let _ = socket.close(None).await;
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("message {text:?} was never stored");
}

/// muted × archived × new message, for every way a chat can be muted.
async fn assert_unarchive_matrix(state: Arc<AppState>, prefix: &str) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    let client = Client::new();

    let sender = register(&client, &base, prefix, "sender").await;
    let room_id = create_chat(&client, &base, &sender.token, &format!("{prefix}-room")).await;
    let other_room = create_chat(&client, &base, &sender.token, &format!("{prefix}-other")).await;
    let hour = chrono::Duration::hours(1);
    // (member, preferences, archived after the message)
    let cases: Vec<(&'static str, Value, bool)> = vec![
        ("plain", json!({ "is_archived": true }), false),
        (
            "mentions",
            json!({ "is_archived": true, "notification_level": "mentions" }),
            false,
        ),
        (
            "expired",
            json!({ "is_archived": true, "muted_until": Utc::now() - hour }),
            false,
        ),
        (
            "silenced",
            json!({ "is_archived": true, "notification_level": "none" }),
            true,
        ),
        (
            "snoozed",
            json!({ "is_archived": true, "muted_until": Utc::now() + hour }),
            true,
        ),
        (
            "mainmuted",
            json!({ "is_archived": false, "notification_level": "none" }),
            false,
        ),
        ("mainplain", json!({ "is_archived": false }), false),
    ];
    let mut members = Vec::new();
    for (name, preferences, expected) in cases {
        let member = register(&client, &base, prefix, name).await;
        join(&client, &base, &member.token, &room_id).await;
        join(&client, &base, &member.token, &other_room).await;
        set_preferences(&client, &base, &member.token, &room_id, preferences.clone()).await;
        set_preferences(
            &client,
            &base,
            &member.token,
            &other_room,
            json!({ "is_archived": true }),
        )
        .await;
        let before = preferences["is_archived"].as_bool().unwrap();
        assert_eq!(
            is_archived(&client, &base, &member, &room_id).await,
            before,
            "{name}: without a new message the archive state is what was set"
        );
        members.push((member, expected));
    }
    // The sender's own message never pulls the chat out of the sender's archive.
    set_preferences(
        &client,
        &base,
        &sender.token,
        &room_id,
        json!({ "is_archived": true }),
    )
    .await;

    send_message(&client, &base, &sender, &room_id, "hello archive").await;

    for (member, expected) in &members {
        assert_eq!(
            is_archived(&client, &base, member, &room_id).await,
            *expected,
            "{}: archived after a new message",
            member.name
        );
        assert!(
            is_archived(&client, &base, member, &other_room).await,
            "{}: a message elsewhere leaves this archive alone",
            member.name
        );
    }
    assert!(
        is_archived(&client, &base, &sender, &room_id).await,
        "own message keeps the sender's chat archived"
    );

    // Unmuting does not replay the message that arrived while muted; the next one counts.
    let (silenced, _) = members.iter().find(|(m, _)| m.name == "silenced").unwrap();
    set_preferences(
        &client,
        &base,
        &silenced.token,
        &room_id,
        json!({ "notification_level": "all" }),
    )
    .await;
    assert!(is_archived(&client, &base, silenced, &room_id).await);
    send_message(&client, &base, &sender, &room_id, "second").await;
    assert!(!is_archived(&client, &base, silenced, &room_id).await);

    task.abort();
}

#[tokio::test]
async fn sqlite_fresh_schema_unarchives_on_new_message() {
    let database = sqlite_scratch_path("unarchive-fresh");
    let state = Arc::new(AppState::open(&database).await.unwrap());
    assert_unarchive_matrix(state.clone(), "uafresh").await;
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgraded_schema_unarchives_on_new_message() {
    let database = sqlite_scratch_path("unarchive-upgrade");
    let pool = sqlite_pool(&database).await;
    migrations_through(&migrations_path("migrations"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let _seeded = seed_sqlite_pre_rename(&pool).await;
    pool.close().await;
    let state = Arc::new(AppState::open(&database).await.unwrap());
    assert_unarchive_matrix(state.clone(), "uaupgrade").await;
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_unarchives_on_new_message() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_unarchives_on_new_message").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "unarchive_fresh").await;
    let state = Arc::new(
        AppState::open_postgres(&scratch.url, &AppConfig::default())
            .await
            .unwrap(),
    );
    assert_unarchive_matrix(state.clone(), "uapgfresh").await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgraded_schema_unarchives_on_new_message() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgraded_schema_unarchives_on_new_message").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "unarchive_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    migrations_through(&migrations_path("migrations-postgres"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let _seeded = seed_postgres_pre_rename(&pool).await;
    pool.close().await;
    let state = Arc::new(
        AppState::open_postgres(&scratch.url, &AppConfig::default())
            .await
            .unwrap(),
    );
    assert_unarchive_matrix(state.clone(), "uapgupgrade").await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_unarchive_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == 20270201000009
                    && migration.description == "unarchive on new message"),
            "{directory} is missing 20270201000009_unarchive_on_new_message"
        );
    }
}
