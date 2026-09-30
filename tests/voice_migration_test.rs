//! TG-401: the voice tables (`20270101000001`) on both adapters, fresh and upgraded.
//!
//! The reserved version sorts *before* migrations that `main` already applied (TG-406's
//! `20270101000006`, TG-505's `20270201…`), so the upgrade that matters is out of order: a
//! database at "everything except 20270101000001" gains it on the next start. Both paths then
//! check the semantics the voice module relies on: the duration CHECK, the source CHECK, one
//! listen per (message, user), and the cascade from `messages`.

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

const VOICE_VERSION: i64 = 20270101000001;

async fn everything_but_voice(directory: &str) -> Migrator {
    let all = Migrator::new(migrations_path(directory).as_path())
        .await
        .unwrap();
    Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|migration| migration.version != VOICE_VERSION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

macro_rules! assert_voice_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let (user, chat, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg401-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg401-chat-{}", chat.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, media_kind, \
             created_at) VALUES ($1, $2, $3, 'tg401', '', 'voice', $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let note = |duration: i64, source: &'static str| {
            sqlx::query(
                "INSERT INTO voice_notes (message_id, duration_ms, waveform, duration_source, \
                 created_at) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(message)
            .bind(duration)
            .bind(vec![0u8; 63])
            .bind(source)
            .bind(now)
        };
        assert!(
            note(0, "container").execute($pool).await.is_err(),
            "duration > 0"
        );
        assert!(
            note(10, "guess").execute($pool).await.is_err(),
            "known sources only"
        );
        note(1_500, "client").execute($pool).await.unwrap();
        let stored: Vec<u8> =
            sqlx::query_scalar("SELECT waveform FROM voice_notes WHERE message_id = $1")
                .bind(message)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(stored.len(), 63, "the packed waveform round-trips as bytes");

        let listen = || {
            sqlx::query(
                "INSERT INTO voice_listens (message_id, user_id, listened_at) VALUES ($1, $2, $3)",
            )
            .bind(message)
            .bind(user)
            .bind(now)
        };
        listen().execute($pool).await.unwrap();
        assert!(
            listen().execute($pool).await.is_err(),
            "one listen per (message, user)"
        );

        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(message)
            .execute($pool)
            .await
            .unwrap();
        for table in ["voice_notes", "voice_listens"] {
            let left: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM {table} WHERE message_id = $1"
            ))
            .bind(message)
            .fetch_one($pool)
            .await
            .unwrap();
            assert_eq!(left, 0, "deleting the message must cascade to {table}");
        }
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_has_voice_tables() {
    let database = sqlite_scratch_path("voice-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_voice_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_out_of_order_upgrade_gains_voice_tables() {
    let database = sqlite_scratch_path("voice-upgrade");
    let pool = sqlite_pool(&database).await;
    everything_but_voice("migrations")
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
    assert_voice_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_voice_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_voice_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "voice_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_voice_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_out_of_order_upgrade_gains_voice_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_out_of_order_upgrade_gains_voice_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "voice_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    everything_but_voice("migrations-postgres")
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
    assert_voice_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_voice_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == VOICE_VERSION
                    && migration.description == "add voice notes"),
            "{directory} is missing 20270101000001_add_voice_notes"
        );
    }
}
