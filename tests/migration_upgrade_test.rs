//! Upgrade-path coverage for the paired migrations on SQLite and PostgreSQL: the FND wave, and
//! TG-004's rename of the Room domain to Chat.
//!
//! The scratch-database plumbing and the TG-004 assertions live in `migration_support` so that
//! both adapters assert the same thing and neither file grows two responsibilities.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use migration_support::{
    chat_data::{
        assert_postgres_backfill, assert_postgres_cascade, assert_sqlite_backfill,
        assert_sqlite_cascade,
    },
    chat_schema::{
        assert_postgres_chat_schema, assert_postgres_foreign_keys_resolve,
        assert_sqlite_chat_schema, assert_sqlite_foreign_keys_resolve, seed_postgres_pre_rename,
        seed_sqlite_pre_rename,
    },
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    migrations_through, postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool,
    sqlite_scratch_path, PRE_TG_004_VERSION,
};
use sqlx::migrate::Migrator;

const PRE_FND_002_VERSION: i64 = 20260826000003;

#[tokio::test]
async fn sqlite_upgrades_from_the_pre_fnd_002_schema() {
    let database = sqlite_scratch_path("sqlite-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    let old = migrations_through(&directory, PRE_FND_002_VERSION).await;
    old.run(&pool).await.unwrap();
    let old_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(old_count, old.iter().count() as i64);
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let full = Migrator::new(directory.as_path()).await.unwrap();
    let upgraded_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(upgraded_count, full.iter().count() as i64);
    let ai_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('ai_thread_messages') \
         WHERE name IN ('stage', 'stage_started_at', 'trace')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(ai_columns, 3);
    let catch_up_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('ai_runs') \
         WHERE name IN ('purpose', 'source_after_message_id', \
           'source_through_message_id', 'source_message_count')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(catch_up_columns, 4);
    let chat_pins: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'chat_pins'",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(chat_pins, 1);
    let chat_tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'chat_tasks'",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(chat_tasks, 1);
    let extraction_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
         ('ai_extraction_runs', 'ai_extraction_candidates', \
          'ai_extraction_candidate_sources', 'ai_extraction_run_candidates')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(extraction_tables, 4);
    let governance_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
         ('chat_ai_policies', 'ai_governance_settings', 'ai_governance_models', \
          'ai_admissions', 'ai_usage_records')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(governance_tables, 5);
    let governed_runs: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM pragma_table_info('ai_runs') WHERE name = 'admission_id') + \
         (SELECT COUNT(*) FROM pragma_table_info('ai_extraction_runs') WHERE name = 'admission_id')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(governed_runs, 2);
    let session_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name IN \
         ('management_id', 'device_name', 'ip_hint', 'last_used_at')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(session_columns, 4);
    let audit_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' \
         AND name IN ('audit_events', 'chat_bans')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(audit_tables, 2);
    let audit_triggers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' \
         AND name IN ('audit_events_reject_update', 'audit_events_reject_delete')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(audit_triggers, 2);

    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_upgrades_from_the_pre_fnd_002_schema() {
    let Some((admin_url, admin_pool)) = postgres_admin_pool().await else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "upgrade").await;
    let database_url = scratch.url.clone();
    let pool = postgres_pool(&database_url).await;
    let directory = migrations_path("migrations-postgres");
    let old = migrations_through(&directory, PRE_FND_002_VERSION).await;
    old.run(&pool).await.unwrap();
    let old_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(old_count, old.iter().count() as i64);
    pool.close().await;

    let state = AppState::open_postgres(&database_url, &AppConfig::default())
        .await
        .unwrap();
    let full = Migrator::new(directory.as_path()).await.unwrap();
    let postgres = state.postgres_pool().unwrap();
    let upgraded_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(upgraded_count, full.iter().count() as i64);
    let ai_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = 'ai_thread_messages' \
         AND column_name IN ('stage', 'stage_started_at', 'trace')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(ai_columns, 3);
    let catch_up_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = 'ai_runs' \
         AND column_name IN ('purpose', 'source_after_message_id', \
           'source_through_message_id', 'source_message_count')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(catch_up_columns, 4);
    let chat_pins: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.chat_pins')::text")
            .fetch_one(postgres)
            .await
            .unwrap();
    assert_eq!(chat_pins.as_deref(), Some("chat_pins"));
    let chat_tasks: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.chat_tasks')::text")
            .fetch_one(postgres)
            .await
            .unwrap();
    assert_eq!(chat_tasks.as_deref(), Some("chat_tasks"));
    let extraction_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' \
         AND table_name IN ('ai_extraction_runs', 'ai_extraction_candidates', \
          'ai_extraction_candidate_sources', 'ai_extraction_run_candidates')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(extraction_tables, 4);
    let governance_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' \
         AND table_name IN ('chat_ai_policies', 'ai_governance_settings', \
          'ai_governance_models', 'ai_admissions', 'ai_usage_records')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(governance_tables, 5);
    let governed_runs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public' \
         AND column_name = 'admission_id' \
         AND table_name IN ('ai_runs', 'ai_extraction_runs')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(governed_runs, 2);
    let session_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public' \
         AND table_name = 'sessions' AND column_name IN \
         ('management_id', 'device_name', 'ip_hint', 'last_used_at')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(session_columns, 4);
    let audit_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' \
         AND table_name IN ('audit_events', 'chat_bans')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(audit_tables, 2);
    let audit_triggers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.triggers WHERE event_object_schema = 'public' \
         AND event_object_table = 'audit_events'",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(audit_triggers, 2);

    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

/// The upgrade every existing deployment actually performs: the schema as it stood at
/// `a16f422` — the last commit before the Telegram-parity programme — carried forward through
/// TG-004's three migrations, with real rows in it.
#[tokio::test]
async fn sqlite_upgrades_from_the_pre_tg_004_schema() {
    let database = sqlite_scratch_path("sqlite-chat-rename");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_sqlite_pre_rename(&pool).await;
    pool.close().await;

    // AppState::open runs the remaining migrations through the production pool, which is the
    // only configuration where SQLite's rename semantics matter (foreign_keys = ON).
    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);

    assert_sqlite_chat_schema(state.pool()).await;
    assert_sqlite_foreign_keys_resolve(state.pool()).await;
    assert_sqlite_backfill(state.pool(), &seeded).await;
    assert_sqlite_cascade(state.pool(), &seeded).await;

    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_upgrades_from_the_pre_tg_004_schema() {
    let Some((admin_url, admin_pool)) = postgres_admin_pool().await else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "chat_rename").await;
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
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);

    assert_postgres_chat_schema(postgres).await;
    assert_postgres_foreign_keys_resolve(postgres).await;
    assert_postgres_backfill(postgres, &seeded).await;
    assert_postgres_cascade(postgres, &seeded).await;

    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
