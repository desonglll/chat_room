//! TG-305: `user_saved_gifs` and `animation_metadata` (`20261201000004`) on both adapters,
//! on a fresh database and on an upgrade from the schema head just before them.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    migrations_through, postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool,
    sqlite_scratch_path,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

/// Everything up to and including the previous M3 migration (TG-304's).
const BEFORE_TG_305: i64 = 20261201000003;

/// A user, as the pre-TG-305 schema knows it.
macro_rules! seed_user {
    ($pool:expr) => {{
        let user = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg305-{}", user.simple()))
        .bind(Utc::now())
        .execute($pool)
        .await
        .unwrap();
        user
    }};
}

/// One saved GIF per (account, content hash); deleting the account removes its GIFs; the
/// geometry table is keyed by content hash.
macro_rules! assert_gif_semantics {
    ($pool:expr, $user:expr) => {{
        let now = Utc::now();
        let insert = "INSERT INTO user_saved_gifs (id, user_id, content_hash, storage_key, \
             mime_type, size_bytes, access_key, source_message_id, saved_at, used_at) \
             VALUES ($1, $2, 'hash', 'hash', 'video/mp4', 10, $3, NULL, $4, $4)";
        sqlx::query(insert)
            .bind(Uuid::new_v4())
            .bind($user)
            .bind(Uuid::new_v4())
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        let duplicate = sqlx::query(insert)
            .bind(Uuid::new_v4())
            .bind($user)
            .bind(Uuid::new_v4())
            .bind(now)
            .execute($pool)
            .await;
        assert!(duplicate.is_err(), "(user_id, content_hash) must be unique");
        sqlx::query(
            "INSERT INTO animation_metadata (content_hash, width, height, duration_ms, \
             created_at) VALUES ('hash', 480, 270, 2000, $1)",
        )
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let width: i64 = sqlx::query_scalar(
            "SELECT md.width FROM user_saved_gifs g \
             JOIN animation_metadata md ON md.content_hash = g.content_hash WHERE g.user_id = $1",
        )
        .bind($user)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(width, 480);
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind($user)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_saved_gifs WHERE user_id = $1")
                .bind($user)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(left, 0, "deleting the account cascades to its saved GIFs");
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_has_gif_tables() {
    let database = sqlite_scratch_path("gifs-fresh");
    let state = AppState::open(&database).await.unwrap();
    let user = seed_user!(state.pool());
    assert_gif_semantics!(state.pool(), user);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_adds_gif_tables_to_an_existing_database() {
    let database = sqlite_scratch_path("gifs-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, BEFORE_TG_305)
        .await
        .run(&pool)
        .await
        .unwrap();
    let user = seed_user!(&pool);
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_gif_semantics!(state.pool(), user);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_gif_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_gif_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "gifs_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let user = seed_user!(postgres);
    assert_gif_semantics!(postgres, user);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_adds_gif_tables_to_an_existing_database() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_adds_gif_tables_to_an_existing_database").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "gifs_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    let directory = migrations_path("migrations-postgres");
    migrations_through(&directory, BEFORE_TG_305)
        .await
        .run(&pool)
        .await
        .unwrap();
    let user = seed_user!(&pool);
    pool.close().await;

    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_gif_semantics!(postgres, user);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn gif_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|m| m.version == 20261201000004 && m.description == "add user saved gifs"),
            "{directory} is missing 20261201000004 add user saved gifs"
        );
    }
}
