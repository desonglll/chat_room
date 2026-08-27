//! Exercises the Postgres backend end to end (create room, WS chat, restart
//! persistence) against a real Postgres server. There is no fake/embedded
//! Postgres to fall back to, so the test creates and drops a throwaway
//! database on a real server rather than touching the app's own database —
//! and skips itself (instead of failing) when no server is reachable, so it
//! doesn't break `cargo test` on machines without Postgres set up.

use super::*;
use chat_room::config::{AdminConfig, AppConfig};

#[path = "ai_extraction_postgres.rs"]
mod ai_extraction_postgres;
#[path = "audit_postgres.rs"]
mod audit_postgres;
#[path = "device_sessions_postgres.rs"]
mod device_sessions_postgres;
#[path = "global_search_postgres.rs"]
mod global_search_postgres;
#[path = "postgres_database.rs"]
mod postgres_database;
#[path = "room_tasks_postgres.rs"]
mod room_tasks_postgres;
use postgres_database::{connect_postgres_admin, create_scratch_database, drop_scratch_database};

#[tokio::test]
async fn postgres_backend_creates_rooms_and_serves_websocket_chat() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_backend_creates_rooms_and_serves_websocket_chat").await
    else {
        return;
    };

    let (db_name, test_url) = create_scratch_database(&admin_pool, &admin_url).await;

    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["pg-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(
        AppState::open_postgres(&test_url, &config)
            .await
            .expect("open_postgres should apply migrations-postgres and connect"),
    );
    let server = start_server_with_state(state.clone()).await;

    let admin_token = super::support::system_admin_token(&state, &server, "pg-admin").await;
    let overview_response = reqwest::Client::new()
        .get(format!("{server}/api/admin/overview"))
        .bearer_auth(admin_token)
        .send()
        .await
        .unwrap();
    let overview_status = overview_response.status();
    let overview_body = overview_response.text().await.unwrap();
    assert_eq!(overview_status, 200, "postgres overview: {overview_body}");
    let overview: serde_json::Value = serde_json::from_str(&overview_body).unwrap();
    assert_eq!(overview["database_backend"], "postgres");
    assert_eq!(
        overview["services"]["items"][0]["state"], "healthy",
        "postgres health probe: {}",
        overview["services"]["items"][0]["detail"]
    );
    assert_eq!(overview["storage"]["logical_bytes"], 0);

    let (room_id, has_password) = create_room(&server, "pg-integration-room", None).await;
    assert!(!has_password);
    let owner_token = session_token(&server, "owner-pg-integration-room").await;
    let preferences: serde_json::Value = reqwest::Client::new()
        .patch(format!("{server}/api/conversations/{room_id}/preferences"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({
            "is_pinned": true,
            "notification_level": "mentions",
            "muted_until": "2030-01-02T03:04:05Z"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preferences["is_pinned"], true);
    assert_eq!(preferences["notification_level"], "mentions");
    let conversation: serde_json::Value = reqwest::Client::new()
        .get(format!("{server}/api/conversations"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap()
        .remove(0);
    assert_eq!(conversation["preferences"], preferences);

    let (mut sink_a, mut stream_a) = ws_connect(&server, &room_id, "alice", None).await;
    let (_sink_b, mut stream_b) = ws_connect(&server, &room_id, "bob", None).await;
    let _ = read_json(&mut stream_a).await; // alice's own join notice
    let _ = read_json(&mut stream_a).await; // bob's join notice
    let _ = read_until_content(&mut stream_b, "bob").await;

    sink_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "hello over postgres" }).to_string(),
        ))
        .await
        .unwrap();
    let broadcast = read_until_type(&mut stream_b, "broadcast").await;
    assert_eq!(broadcast["sender"], "alice");
    assert_eq!(broadcast["content"], "hello over postgres");

    let stored_rooms: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rooms")
        .fetch_one(state.postgres_pool().expect("postgres-backed state"))
        .await
        .unwrap();
    assert_eq!(stored_rooms, 1);
    let stored_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(state.postgres_pool().expect("postgres-backed state"))
        .await
        .unwrap();
    assert_eq!(stored_messages, 1);

    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);

    drop_scratch_database(&admin_pool, &db_name).await;
}

/// `migrations/` (SQLite) and `migrations-postgres/` (Postgres) are maintained
/// as separate files by hand — SQLite's early migrations are consolidated into
/// one `..._initial.sql` on the Postgres side, so comparing file contents
/// directly isn't meaningful. Instead this applies both migration sets to
/// fresh databases and compares the resulting tables/columns, which catches
/// the failure mode that actually matters: the two backends drifting apart.
#[tokio::test]
async fn sqlite_and_postgres_migrations_produce_matching_schemas() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("sqlite_and_postgres_migrations_produce_matching_schemas").await
    else {
        return;
    };

    let (db_name, test_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let pg_state = AppState::open_postgres(&test_url, &AppConfig::default())
        .await
        .expect("open_postgres should apply migrations-postgres and connect");
    let sqlite_state = AppState::new()
        .await
        .expect("AppState::new should apply migrations and connect");

    let pg_pool = pg_state.postgres_pool().expect("postgres-backed state");

    let sqlite_tables: std::collections::BTreeSet<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' \
         AND name NOT LIKE 'sqlx_%' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx%'",
    )
    .fetch_all(sqlite_state.pool())
    .await
    .unwrap()
    .into_iter()
    .collect();

    let pg_tables: std::collections::BTreeSet<String> = sqlx::query_scalar(
        "SELECT tablename FROM pg_tables WHERE schemaname = 'public' AND tablename NOT LIKE '\\_sqlx%'",
    )
    .fetch_all(pg_pool)
    .await
    .unwrap()
    .into_iter()
    .collect();

    assert_eq!(
        sqlite_tables, pg_tables,
        "migrations/ and migrations-postgres/ must create the same set of tables"
    );

    for table in &sqlite_tables {
        let sqlite_columns: std::collections::BTreeSet<String> =
            sqlx::query_scalar(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(sqlite_state.pool())
                .await
                .unwrap()
                .into_iter()
                .collect();

        let pg_columns: std::collections::BTreeSet<String> = sqlx::query_scalar(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = $1",
        )
        .bind(table)
        .fetch_all(pg_pool)
        .await
        .unwrap()
        .into_iter()
        .collect();

        assert_eq!(
            sqlite_columns, pg_columns,
            "column set mismatch between migrations/ and migrations-postgres/ for table `{table}`"
        );
    }

    pg_state.postgres_pool().unwrap().close().await;
    drop(pg_state);

    drop_scratch_database(&admin_pool, &db_name).await;
}

#[tokio::test]
async fn postgres_friendship_creates_one_private_direct_conversation() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_friendship_creates_one_private_direct_conversation").await
    else {
        return;
    };
    let (db_name, test_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&test_url, &AppConfig::default())
            .await
            .expect("open postgres social database"),
    );
    let server = start_server_with_state(state.clone()).await;
    let client = reqwest::Client::new();
    let alice_token = session_token(&server, "pg-social-alice").await;
    let bob_token = session_token(&server, "pg-social-bob").await;

    let search: Vec<serde_json::Value> = client
        .get(format!("{server}/api/users/search?q=pg-social-bob"))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bob_id = search[0]["id"].as_str().unwrap();
    assert_eq!(search[0]["relationship"], "none");
    assert_eq!(
        client
            .post(format!("{server}/api/friend-requests"))
            .bearer_auth(&alice_token)
            .json(&serde_json::json!({ "user_id": bob_id }))
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    let incoming: Vec<serde_json::Value> = client
        .get(format!("{server}/api/friend-requests?direction=incoming"))
        .bearer_auth(&bob_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let alice_id = incoming[0]["user"]["id"].as_str().unwrap();
    assert_eq!(
        client
            .patch(format!("{server}/api/friend-requests/{alice_id}"))
            .bearer_auth(&bob_token)
            .json(&serde_json::json!({ "action": "accept" }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let alice_start = client
        .post(format!("{server}/api/direct-chats"))
        .bearer_auth(&alice_token)
        .json(&serde_json::json!({ "user_id": bob_id }))
        .send();
    let bob_start = client
        .post(format!("{server}/api/direct-chats"))
        .bearer_auth(&bob_token)
        .json(&serde_json::json!({ "user_id": alice_id }))
        .send();
    let (alice_response, bob_response) = tokio::join!(alice_start, bob_start);
    let alice_response = alice_response.unwrap();
    let bob_response = bob_response.unwrap();
    assert_eq!(alice_response.status(), 200);
    assert_eq!(bob_response.status(), 200);
    let conversation: serde_json::Value = alice_response.json().await.unwrap();
    let peer_conversation: serde_json::Value = bob_response.json().await.unwrap();
    assert_eq!(conversation["kind"], "direct");
    assert_eq!(conversation["title"], "pg-social-bob");
    assert_eq!(conversation["room_id"], peer_conversation["room_id"]);
    let room_id = conversation["room_id"].as_str().unwrap();

    let (mut direct_sink, mut direct_stream) =
        ws_connect(&server, room_id, "pg-social-alice", None).await;
    direct_sink
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "private postgres source" })
                .to_string(),
        ))
        .await
        .unwrap();
    let source_message = read_until_type(&mut direct_stream, "broadcast").await;
    let target_room: serde_json::Value = client
        .post(format!("{server}/api/rooms"))
        .bearer_auth(&alice_token)
        .json(&serde_json::json!({ "name": "pg-forward-target" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let forwarded: Vec<serde_json::Value> = client
        .post(format!("{server}/api/messages/forward"))
        .bearer_auth(&alice_token)
        .json(&serde_json::json!({
            "message_ids": [source_message["message_id"]],
            "target_room_ids": [target_room["id"]]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(forwarded[0]["forwarded_message_id"].is_string());
    let target_messages: Vec<serde_json::Value> = client
        .get(format!(
            "{server}/api/rooms/{}/messages",
            target_room["id"].as_str().unwrap()
        ))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        target_messages[0]["forwarded_from"]["room_name"],
        "pg-social-bob"
    );

    let direct_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM direct_conversations")
        .fetch_one(state.postgres_pool().unwrap())
        .await
        .unwrap();
    let member_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM room_memberships WHERE room_id = $1")
            .bind(room_id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(state.postgres_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(direct_count, 1);
    assert_eq!(member_count, 2);

    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_scratch_database(&admin_pool, &db_name).await;
}
