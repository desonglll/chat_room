//! Exercises the Postgres backend end to end (create room, WS chat, restart
//! persistence) against a real Postgres server. There is no fake/embedded
//! Postgres to fall back to, so the test creates and drops a throwaway
//! database on a real server rather than touching the app's own database —
//! and skips itself (instead of failing) when no server is reachable, so it
//! doesn't break `cargo test` on machines without Postgres set up.

use super::*;
use chat_room::config::{AdminConfig, AppConfig};
use sqlx::postgres::PgPoolOptions;

fn postgres_admin_url() -> String {
    std::env::var("TEST_POSTGRES_ADMIN_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:52735/postgres".to_string())
}

/// Create a fresh, empty database on the same server as `admin_url` and
/// return its connection URL. The caller is responsible for dropping it.
async fn create_scratch_database(admin_pool: &sqlx::PgPool, admin_url: &str) -> (String, String) {
    let db_name = format!("chat_room_test_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{db_name}""#))
        .execute(admin_pool)
        .await
        .unwrap();
    let base = admin_url
        .rsplit_once('/')
        .map(|(head, _)| head)
        .unwrap_or(admin_url);
    (db_name.clone(), format!("{base}/{db_name}"))
}

#[tokio::test]
async fn postgres_backend_creates_rooms_and_serves_websocket_chat() {
    let admin_url = postgres_admin_url();
    let admin_pool = match PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!(
                "skipping postgres_backend_creates_rooms_and_serves_websocket_chat: \
                 could not reach Postgres at {admin_url}: {error}"
            );
            return;
        }
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

    let admin_token = session_token(&server, "pg-admin").await;
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
    assert_eq!(overview["storage"]["logical_bytes"], 0);

    let (room_id, has_password) = create_room(&server, "pg-integration-room", None).await;
    assert!(!has_password);

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

    // `server.shutdown()` aborts the top-level accept loop, but hyper spawns
    // each already-open connection (including each WS handler's DB-polling
    // loop) as its own independent task, so a couple of scratch-DB sessions
    // can still be winding down here — terminate them before dropping.
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&db_name)
    .execute(&admin_pool)
    .await
    .ok();

    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("drop scratch database");
}

/// `migrations/` (SQLite) and `migrations-postgres/` (Postgres) are maintained
/// as separate files by hand — SQLite's early migrations are consolidated into
/// one `..._initial.sql` on the Postgres side, so comparing file contents
/// directly isn't meaningful. Instead this applies both migration sets to
/// fresh databases and compares the resulting tables/columns, which catches
/// the failure mode that actually matters: the two backends drifting apart.
#[tokio::test]
async fn sqlite_and_postgres_migrations_produce_matching_schemas() {
    let admin_url = postgres_admin_url();
    let admin_pool = match PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!(
                "skipping sqlite_and_postgres_migrations_produce_matching_schemas: \
                 could not reach Postgres at {admin_url}: {error}"
            );
            return;
        }
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

    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&db_name)
    .execute(&admin_pool)
    .await
    .ok();

    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{db_name}""#))
        .execute(&admin_pool)
        .await
        .expect("drop scratch database");
}
