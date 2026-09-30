//! TG-111: the in-memory test database must survive pooled-connection churn.
//!
//! `AppState::new()` runs on `storage::open_memory_database`, a one-connection pool. When a
//! task is cancelled mid-query sqlx discards that connection and opens a new one. With a
//! private `:memory:` database the replacement was a brand-new EMPTY database, which made
//! WebSocket-heavy suites fail intermittently with "no such table: sessions".

use std::time::Duration;

use chat_room::attachment_storage::{test_directory, AttachmentStore};
use chat_room::config::OssConfig;
use chat_room::storage::open_memory_database;
use sqlx::{Connection, SqlitePool};

async fn memory_pool() -> SqlitePool {
    let store = AttachmentStore::open(
        test_directory(),
        Duration::from_secs(3600),
        &OssConfig::default(),
    )
    .await
    .unwrap();
    open_memory_database(&store).await.unwrap()
}

async fn seed(pool: &SqlitePool) {
    sqlx::query("CREATE TABLE churn_probe (value INTEGER NOT NULL)")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO churn_probe (value) VALUES (42)")
        .execute(pool)
        .await
        .unwrap();
}

async fn assert_survived(pool: &SqlitePool) {
    let value: i64 = sqlx::query_scalar("SELECT value FROM churn_probe")
        .fetch_one(pool)
        .await
        .expect("seeded table must survive a replaced pool connection");
    assert_eq!(value, 42);
    let migrated: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = 'sessions'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(migrated, 1, "migrated schema must survive too");
}

/// Deterministic reproduction: the pool's only connection is closed and replaced.
#[tokio::test]
async fn data_survives_when_the_only_pooled_connection_is_replaced() {
    let pool = memory_pool().await;
    seed(&pool).await;

    for _ in 0..3 {
        let connection = pool.acquire().await.unwrap().detach();
        connection.close().await.unwrap();
        assert_survived(&pool).await;
    }
}

/// The production failure shape: a task is aborted while its query is running.
#[tokio::test]
async fn data_survives_a_task_aborted_mid_query() {
    let pool = memory_pool().await;
    seed(&pool).await;

    for _ in 0..2 {
        let busy = pool.clone();
        let task = tokio::spawn(async move {
            // A deliberately long recursive query so the abort lands mid-statement.
            let _ = sqlx::query_scalar::<_, i64>(
                "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n \
                 WHERE x < 1000000) SELECT COUNT(*) FROM n",
            )
            .fetch_one(&busy)
            .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        task.abort();
        let _ = task.await;
        assert_survived(&pool).await;
    }
}

/// Two pools never share a database: the unique name keeps tests isolated.
#[tokio::test]
async fn separate_memory_databases_are_isolated() {
    let first = memory_pool().await;
    let second = memory_pool().await;
    seed(&first).await;
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = 'churn_probe'")
            .fetch_one(&second)
            .await
            .unwrap();
    assert_eq!(exists, 0);
}

/// The URI must be parsed as a memory database, never created as a file on disk.
#[tokio::test]
async fn memory_database_never_touches_the_working_directory() {
    let _pool = memory_pool().await;
    let leaked = std::fs::read_dir(".")
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("chat-room-memory")
        });
    assert!(
        !leaked,
        "in-memory database leaked a file into the working directory"
    );
}
