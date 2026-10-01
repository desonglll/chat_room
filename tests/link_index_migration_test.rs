//! TG-803: the link index tables (`20270301000001`) on both adapters, fresh and upgraded from
//! the schema before them. Checks what `messages::shared_links` relies on: one row per
//! (message, position), one index state per chat, and the cascade from `messages`/`chats`.

mod migration_support;

use std::borrow::Cow;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool, sqlite_scratch_path,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

const LINKS_VERSION: i64 = 20270301000001;

async fn everything_but_links(directory: &str) -> Migrator {
    let all = Migrator::new(migrations_path(directory).as_path())
        .await
        .unwrap();
    Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|migration| migration.version != LINKS_VERSION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

macro_rules! assert_link_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let (user, chat, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg803-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg803-chat-{}", chat.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES ($1, $2, $3, 'tg803', 'https://a.example/', $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let link = |position: i32| {
            sqlx::query(
                "INSERT INTO message_links (message_id, position, room_id, url, created_at) \
                 VALUES ($1, $2, $3, 'https://a.example/', $4)",
            )
            .bind(message)
            .bind(position)
            .bind(chat)
            .bind(now)
        };
        link(0).execute($pool).await.unwrap();
        assert!(
            link(0).execute($pool).await.is_err(),
            "one row per (message, position)"
        );
        let state = || {
            sqlx::query(
                "INSERT INTO chat_link_index_state \
                 (room_id, indexed_created_at, indexed_message_id, edits_seen_at) \
                 VALUES ($1, $2, $3, $2)",
            )
            .bind(chat)
            .bind(now)
            .bind(message)
        };
        state().execute($pool).await.unwrap();
        assert!(
            state().execute($pool).await.is_err(),
            "one index state per chat"
        );

        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(message)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message_links WHERE room_id = $1")
            .bind(chat)
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(left, 0, "deleting the message must cascade to its links");
        sqlx::query("DELETE FROM chats WHERE id = $1")
            .bind(chat)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chat_link_index_state WHERE room_id = $1")
                .bind(chat)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(left, 0, "deleting the chat must cascade to its index state");
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_has_link_index_tables() {
    let database = sqlite_scratch_path("links-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_link_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_gains_link_index_tables() {
    let database = sqlite_scratch_path("links-upgrade");
    let pool = sqlite_pool(&database).await;
    everything_but_links("migrations")
        .await
        .run(&pool)
        .await
        .unwrap();
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations")).await
    );
    assert_link_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_link_index_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_link_index_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "links_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_link_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_gains_link_index_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_gains_link_index_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "links_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    everything_but_links("migrations-postgres")
        .await
        .run(&pool)
        .await
        .unwrap();
    pool.close().await;

    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations-postgres")).await
    );
    assert_link_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_link_index_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == LINKS_VERSION
                    && migration.description == "add message links"),
            "{directory} is missing 20270301000001_add_message_links"
        );
    }
}
