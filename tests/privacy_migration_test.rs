//! TG-505: `20270201000004_add_user_privacy` on both adapters, fresh and upgraded from the
//! pre-TG-004 schema every real deployment starts from. Both paths then exercise the
//! constraints the privacy module relies on: the CHECKs on dimension / tier / effect, the
//! one-exception-per-account primary key, the no-self-exception CHECK, and the cascades.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::{NaiveDate, Utc};
use migration_support::{
    chat_schema::{seed_postgres_pre_rename, seed_sqlite_pre_rename},
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    migrations_through, postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool,
    sqlite_scratch_path, PRE_TG_004_VERSION,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

const TABLES: [&str; 3] = [
    "user_privacy_rules",
    "user_privacy_exceptions",
    "user_last_seen",
];

macro_rules! assert_privacy_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let (owner, other) = (Uuid::new_v4(), Uuid::new_v4());
        for user in [owner, other] {
            sqlx::query(
                "INSERT INTO users (id, username, password_hash, created_at) \
                 VALUES ($1, $2, 'x', $3)",
            )
            .bind(user)
            .bind(format!("tg505-{}", user.simple()))
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        let rule = |key: &'static str, tier: &'static str| {
            sqlx::query(
                "INSERT INTO user_privacy_rules (user_id, privacy_key, tier, updated_at) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(owner)
            .bind(key)
            .bind(tier)
            .bind(now)
        };
        rule("last_seen", "contacts").execute($pool).await.unwrap();
        assert!(
            rule("last_seen", "nobody").execute($pool).await.is_err(),
            "one rule per key"
        );
        assert!(
            rule("phone_number", "nobody").execute($pool).await.is_err(),
            "no phone key"
        );
        assert!(
            rule("forwards", "friends").execute($pool).await.is_err(),
            "tier CHECK"
        );

        let exception = |target: Uuid, effect: &'static str| {
            sqlx::query(
                "INSERT INTO user_privacy_exceptions \
                 (user_id, privacy_key, target_user_id, effect, created_at) \
                 VALUES ($1, 'last_seen', $2, $3, $4)",
            )
            .bind(owner)
            .bind(target)
            .bind(effect)
            .bind(now)
        };
        exception(other, "allow").execute($pool).await.unwrap();
        assert!(
            exception(other, "deny").execute($pool).await.is_err(),
            "one list per account"
        );
        assert!(
            exception(owner, "deny").execute($pool).await.is_err(),
            "no self exception"
        );

        let day = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        sqlx::query(
            "INSERT INTO user_last_seen (user_id, last_seen_at, prior_seen_day) \
             VALUES ($1, $2, $3)",
        )
        .bind(other)
        .bind(now)
        .bind(day)
        .execute($pool)
        .await
        .unwrap();
        let stored: Option<NaiveDate> =
            sqlx::query_scalar("SELECT prior_seen_day FROM user_last_seen WHERE user_id = $1")
                .bind(other)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(stored, Some(day));

        // Deleting the exception's target removes the exception; deleting the owner
        // removes the rule; deleting an account removes its last-seen.
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(other)
            .execute($pool)
            .await
            .unwrap();
        let exceptions: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_privacy_exceptions WHERE user_id = $1")
                .bind(owner)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(exceptions, 0);
        let seen: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_last_seen WHERE user_id = $1")
                .bind(other)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(seen, 0);
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(owner)
            .execute($pool)
            .await
            .unwrap();
        let rules: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM user_privacy_rules WHERE user_id = $1")
                .bind(owner)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(rules, 0);
    }};
}

async fn assert_sqlite_tables(pool: &sqlx::SqlitePool) {
    for table in TABLES {
        let found: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = $1",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(found, 1, "{table}");
    }
}

async fn assert_postgres_tables(pool: &sqlx::PgPool) {
    for table in TABLES {
        let found: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_name = $1",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(found, 1, "{table}");
    }
}

#[tokio::test]
async fn sqlite_fresh_schema_has_the_privacy_tables() {
    let database = sqlite_scratch_path("privacy-fresh");
    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations")).await
    );
    assert_sqlite_tables(state.pool()).await;
    assert_privacy_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_from_the_pre_tg_004_schema_gains_the_privacy_tables() {
    let database = sqlite_scratch_path("privacy-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let _seeded = seed_sqlite_pre_rename(&pool).await;
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_sqlite_tables(state.pool()).await;
    assert_privacy_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_the_privacy_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_the_privacy_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "privacy_fresh").await;
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
    assert_postgres_tables(postgres).await;
    assert_privacy_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_from_the_pre_tg_004_schema_gains_the_privacy_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_from_the_pre_tg_004_schema_gains_the_privacy_tables")
            .await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "privacy_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    let directory = migrations_path("migrations-postgres");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let _seeded = seed_postgres_pre_rename(&pool).await;
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
    assert_postgres_tables(postgres).await;
    assert_privacy_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_privacy_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == 20270201000004
                    && migration.description == "add user privacy"),
            "{directory} is missing 20270201000004_add_user_privacy"
        );
    }
}
