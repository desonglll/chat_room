//! TG-004 on a database created from nothing, for both adapters.
//!
//! The upgrade path lives in `migration_upgrade_test.rs`. This file answers the other half of
//! `AGENTS.md`'s database rule: a brand-new deployment gets the same schema as an upgraded one.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use migration_support::{
    chat_schema::{
        assert_postgres_chat_schema, assert_postgres_foreign_keys_resolve,
        assert_sqlite_chat_schema, assert_sqlite_foreign_keys_resolve, RENAMED_AWAY, RENAMED_TO,
    },
    create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool, remove_sqlite_files,
    sqlite_scratch_path,
};

#[tokio::test]
async fn a_fresh_sqlite_database_has_the_chat_schema_and_no_dangling_foreign_key() {
    let database = sqlite_scratch_path("sqlite-fresh-chats");
    let state = AppState::open(&database).await.unwrap();

    assert_sqlite_chat_schema(state.pool()).await;
    assert_sqlite_foreign_keys_resolve(state.pool()).await;

    // The nine permission keys the registry ships with survive the table rename: the rows are
    // user-facing data, not schema, and `chat_role_permissions` points at them by value.
    let permissions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_permissions")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert!(
        permissions >= 9,
        "the permission registry lost rows in the rename: {permissions}"
    );
    let settings_key: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_permissions WHERE permission_key = $1")
            .bind("room.settings")
            .fetch_one(state.pool())
            .await
            .unwrap();
    assert_eq!(
        settings_key, 1,
        "permission key values are frozen until M2 expands the registry"
    );

    // chat_type is constrained to the four documented values and to nothing else.
    let rejected = sqlx::query(
        "INSERT INTO chats (id, chat_type, title, access_hash, created_at) \
         VALUES ($1, 'megagroup', 'bad', 'deadbeefdeadbeef', $2)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(chrono::Utc::now())
    .execute(state.pool())
    .await;
    assert!(
        rejected.is_err(),
        "chats.chat_type accepted a value outside its CHECK constraint"
    );

    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn the_active_username_index_is_unique_only_among_live_chats() {
    let database = sqlite_scratch_path("sqlite-username-idx");
    let state = AppState::open(&database).await.unwrap();
    let now = chrono::Utc::now();
    let insert = |id: uuid::Uuid, title: &'static str, deleted: bool| {
        sqlx::query(
            "INSERT INTO chats (id, chat_type, title, access_hash, username, deleted_at, \
             created_at) VALUES ($1, 'supergroup', $2, 'deadbeefdeadbeef', 'shared', $3, $4)",
        )
        .bind(id)
        .bind(title)
        .bind(deleted.then_some(now))
        .bind(now)
        .execute(state.pool())
    };

    insert(uuid::Uuid::new_v4(), "live", false).await.unwrap();
    assert!(
        insert(uuid::Uuid::new_v4(), "second-live", false)
            .await
            .is_err(),
        "two live chats claimed the same public username"
    );
    insert(uuid::Uuid::new_v4(), "archived", true)
        .await
        .expect("a soft-deleted chat may keep a username another chat now uses");

    // A NULL username is the normal case and must never collide.
    for title in ["no-handle-a", "no-handle-b"] {
        sqlx::query(
            "INSERT INTO chats (id, chat_type, title, access_hash, created_at) \
             VALUES ($1, 'group', $2, 'deadbeefdeadbeef', $3)",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(title)
        .bind(now)
        .execute(state.pool())
        .await
        .unwrap();
    }

    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn a_fresh_postgres_database_has_the_chat_schema_and_no_stale_trigger_function() {
    let Some((admin_url, admin_pool)) = postgres_admin_pool().await else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "fresh_chats").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();

    assert_postgres_chat_schema(postgres).await;
    assert_postgres_foreign_keys_resolve(postgres).await;

    let settings_key: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_permissions WHERE permission_key = $1")
            .bind("room.settings")
            .fetch_one(postgres)
            .await
            .unwrap();
    assert_eq!(settings_key, 1);

    let rejected = sqlx::query(
        "INSERT INTO chats (id, chat_type, title, access_hash, created_at) \
         VALUES ($1, 'megagroup', 'bad', 'deadbeefdeadbeef', $2)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(chrono::Utc::now())
    .execute(postgres)
    .await;
    assert!(rejected.is_err());

    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[test]
fn the_rename_table_is_exhaustive_and_has_no_overlap() {
    // room_participants is the one member of the family with no new name: it was dropped by
    // 20260818000018 before TG-004 existed, in both adapters.
    assert_eq!(RENAMED_AWAY.len(), RENAMED_TO.len() + 1);
    for old in RENAMED_AWAY {
        assert!(!RENAMED_TO.contains(&old), "{old} is both old and new");
    }
}
