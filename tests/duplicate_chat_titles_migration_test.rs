//! TG-1210: `20271001000001` drops the unique index on active chat titles, on both adapters,
//! fresh and upgraded from the schema before it. The public @username stays unique.

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

const DUPLICATE_TITLES_VERSION: i64 = 20271001000001;

async fn everything_but_duplicate_titles(directory: &str) -> Migrator {
    let all = Migrator::new(migrations_path(directory).as_path())
        .await
        .unwrap();
    Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|migration| migration.version != DUPLICATE_TITLES_VERSION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

/// Insert a user and an active chat titled `title`, optionally with a public handle.
macro_rules! insert_chat {
    ($pool:expr, $title:expr, $username:expr) => {{
        let now = Utc::now();
        let (user, chat) = (Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg1210-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             username, created_at) VALUES ($1, $2, '', $3, 'open', $4, $5)",
        )
        .bind(chat)
        .bind($title)
        .bind(user)
        .bind($username)
        .bind(now)
        .execute($pool)
        .await
        .map(|_| chat)
    }};
}

macro_rules! assert_titles_repeat_handles_do_not {
    ($pool:expr, $title:expr) => {{
        let first = insert_chat!($pool, $title, None::<String>).unwrap();
        let second = insert_chat!($pool, $title, None::<String>)
            .expect("two active chats may share a title");
        assert_ne!(first, second);
        let same: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM chats WHERE title = $1 AND deleted_at IS NULL",
        )
        .bind($title)
        .fetch_one($pool)
        .await
        .unwrap();
        assert!(same >= 2, "both chats titled {:?} are kept", $title);

        let handle = format!("h{}", Uuid::new_v4().simple());
        insert_chat!($pool, "handle one", Some(handle.clone())).unwrap();
        assert!(
            insert_chat!($pool, "handle two", Some(handle)).is_err(),
            "the public @username stays unique"
        );
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_allows_duplicate_titles() {
    let database = sqlite_scratch_path("dup-titles-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_titles_repeat_handles_do_not!(state.pool(), "lobby");
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_drops_the_title_uniqueness() {
    let database = sqlite_scratch_path("dup-titles-upgrade");
    let pool = sqlite_pool(&database).await;
    everything_but_duplicate_titles("migrations")
        .await
        .run(&pool)
        .await
        .unwrap();
    let kept = insert_chat!(&pool, "lobby", None::<String>).unwrap();
    assert!(
        insert_chat!(&pool, "lobby", None::<String>).is_err(),
        "before the migration a repeated title is refused"
    );
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
    let survived: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chats WHERE id = $1")
        .bind(kept)
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(survived, 1, "existing chats survive the upgrade");
    assert_titles_repeat_handles_do_not!(state.pool(), "lobby");
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_allows_duplicate_titles() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_allows_duplicate_titles").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "dup_titles_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_titles_repeat_handles_do_not!(postgres, "lobby");
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_drops_the_title_uniqueness() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_drops_the_title_uniqueness").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "dup_titles_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    everything_but_duplicate_titles("migrations-postgres")
        .await
        .run(&pool)
        .await
        .unwrap();
    let kept = insert_chat!(&pool, "lobby", None::<String>).unwrap();
    assert!(
        insert_chat!(&pool, "lobby", None::<String>).is_err(),
        "before the migration a repeated title is refused"
    );
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
    let survived: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chats WHERE id = $1")
        .bind(kept)
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(survived, 1, "existing chats survive the upgrade");
    assert_titles_repeat_handles_do_not!(postgres, "lobby");
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_duplicate_titles_migration_exists_in_both_adapters() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == DUPLICATE_TITLES_VERSION
                    && migration.description == "allow duplicate chat titles"),
            "{directory} is missing 20271001000001_allow_duplicate_chat_titles"
        );
    }
}
