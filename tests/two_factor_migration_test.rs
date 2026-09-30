//! TG-506: `20270201000005_add_two_factor_credentials` on both adapters, fresh and upgraded,
//! plus the whole 2FA flow against PostgreSQL (the SQL uses `ON CONFLICT`, `RETURNING` and
//! typed NULL binds, so SQLite passing proves nothing about PostgreSQL).
//!
//! Upgrade path: the pre-TG-004 schema with seeded accounts, carried through every later
//! migration in one go. Seeded accounts have no `two_factor_credentials` row, which is
//! exactly "2FA off" — nothing to backfill.

mod migration_support;
#[allow(dead_code)]
mod two_factor_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    chat_schema::{seed_postgres_pre_rename, seed_sqlite_pre_rename},
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    migrations_through, postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool,
    sqlite_scratch_path, PRE_TG_004_VERSION,
};
use reqwest::{Client, StatusCode};
use sqlx::migrate::Migrator;
use two_factor_support::*;
use uuid::Uuid;

const TABLES: [&str; 3] = [
    "two_factor_credentials",
    "two_factor_email_codes",
    "two_factor_login_challenges",
];

/// Rows for one account in all three tables; the `purpose` CHECK rejects junk; deleting
/// the account cascades everywhere. Pure SQL so it runs identically on both pools.
macro_rules! assert_two_factor_semantics {
    ($pool:expr, $user:expr) => {{
        let now = Utc::now();
        let user: Uuid = $user;
        sqlx::query(
            "INSERT INTO two_factor_credentials \
             (user_id, password_hash, hint, recovery_email, created_at, updated_at) \
             VALUES ($1, 'h', '', NULL, $2, $2)",
        )
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO two_factor_email_codes \
             (user_id, purpose, email, code_hash, attempts, expires_at, created_at) \
             VALUES ($1, 'reset', 'a@b.cd', 'h', 0, $2, $2)",
        )
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let junk = sqlx::query(
            "INSERT INTO two_factor_email_codes \
             (user_id, purpose, email, code_hash, attempts, expires_at, created_at) \
             VALUES ($1, 'anything', 'a@b.cd', 'h', 0, $2, $2)",
        )
        .bind(user)
        .bind(now)
        .execute($pool)
        .await;
        assert!(junk.is_err(), "purpose must be verify_email or reset");
        sqlx::query(
            "INSERT INTO two_factor_login_challenges \
             (token_hash, user_id, attempts, expires_at, created_at) VALUES ($1, $2, 0, $3, $3)",
        )
        .bind(Uuid::new_v4().simple().to_string())
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user)
            .execute($pool)
            .await
            .unwrap();
        for table in TABLES {
            let left: i64 =
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE user_id = $1"))
                    .bind(user)
                    .fetch_one($pool)
                    .await
                    .unwrap();
            assert_eq!(left, 0, "deleting the account must cascade to {table}");
        }
    }};
}

macro_rules! fresh_user {
    ($pool:expr) => {{
        let user = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg506-{}", user.simple()))
        .bind(Utc::now())
        .execute($pool)
        .await
        .unwrap();
        user
    }};
}

async fn sqlite_applied(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn postgres_applied(pool: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn sqlite_fresh_schema_has_two_factor_tables() {
    let database = sqlite_scratch_path("two-factor-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_eq!(
        sqlite_applied(state.pool()).await,
        migration_count(&migrations_path("migrations")).await
    );
    let user = fresh_user!(state.pool());
    assert_two_factor_semantics!(state.pool(), user);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_keeps_existing_accounts_without_two_factor() {
    let database = sqlite_scratch_path("two-factor-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_sqlite_pre_rename(&pool).await;
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    assert_eq!(
        sqlite_applied(state.pool()).await,
        migration_count(&directory).await
    );
    let enrolled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM two_factor_credentials")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(
        enrolled, 0,
        "no existing account is enrolled by the upgrade"
    );
    let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id IN ($1, $2)")
        .bind(seeded.owner)
        .bind(seeded.peer)
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(kept, 2);
    let user = fresh_user!(state.pool());
    assert_two_factor_semantics!(state.pool(), user);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_runs_the_whole_two_factor_flow() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_runs_the_whole_two_factor_flow").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "two_factor_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap().clone();
    assert_eq!(
        postgres_applied(&postgres).await,
        migration_count(&migrations_path("migrations-postgres")).await
    );
    let user = fresh_user!(&postgres);
    assert_two_factor_semantics!(&postgres, user);

    let server = start(state).await;
    let client = Client::new();
    let username = unique("pg-flow");
    let email = format!("{username}@example.com");
    let first = register(&client, &server, &username).await;
    let current = token_of(login(&client, &server, &username).await).await;
    assert_eq!(
        enable(&client, &server, &current, "pg hint").await.status(),
        StatusCode::OK
    );
    assert_eq!(
        me_status(&client, &server, &first).await,
        StatusCode::UNAUTHORIZED
    );
    let changed = authed(
        &client,
        &server,
        reqwest::Method::PUT,
        "/api/users/me/two-factor",
        &current,
        Some(serde_json::json!({ "current_password": TWO_FA_PASSWORD, "hint": "pg hint 2" })),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);
    verify_recovery_email(&client, &server, &current, &email).await;

    let challenge = challenge(&client, &server, &username).await;
    assert_eq!(challenge["hint"], "pg hint 2");
    let pending = challenge["pending_token"].as_str().unwrap().to_string();
    assert_eq!(
        second_stage(&client, &server, &pending, "wrong-second-pw")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let recovery = post(
        &client,
        &server,
        "/api/users/login/two-factor/recovery",
        serde_json::json!({ "pending_token": pending }),
    )
    .await;
    assert_eq!(recovery.status(), StatusCode::ACCEPTED);
    let recovered = post(
        &client,
        &server,
        "/api/users/login/two-factor/recovery/confirm",
        serde_json::json!({ "pending_token": pending, "code": latest_code(&email) }),
    )
    .await;
    assert_eq!(recovered.status(), StatusCode::OK);
    assert_eq!(
        me_status(&client, &server, &current).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&client, &server, &username).await.status(),
        StatusCode::OK
    );

    drop(server);
    postgres.close().await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_keeps_existing_accounts_without_two_factor() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_keeps_existing_accounts_without_two_factor").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "two_factor_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    let directory = migrations_path("migrations-postgres");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_postgres_pre_rename(&pool).await;
    pool.close().await;

    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_eq!(
        postgres_applied(postgres).await,
        migration_count(&directory).await
    );
    let enrolled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM two_factor_credentials")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(enrolled, 0);
    let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id IN ($1, $2)")
        .bind(seeded.owner)
        .bind(seeded.peer)
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(kept, 2);
    let user = fresh_user!(postgres);
    assert_two_factor_semantics!(postgres, user);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|m| m.version == 20270201000005
                    && m.description == "add two factor credentials"),
            "{directory} is missing 20270201000005_add_two_factor_credentials"
        );
    }
}
