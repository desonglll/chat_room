//! TG-204: migration `20261101000005` (forum topics) on both adapters — on a fresh schema, and
//! upgraded from the pre-TG-004 baseline with a seeded chat and message, which must land in
//! General (`topic_id IS NULL`) untouched.

mod migration_support;

use chat_room::{config::AppConfig, models::Chat, state::AppState};
use chrono::Utc;
use migration_support::{
    chat_schema::{seed_postgres_pre_rename, seed_sqlite_pre_rename, SeededChats},
    create_postgres_scratch, drop_postgres_scratch, migrations_path, migrations_through,
    postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool, sqlite_scratch_path,
    PRE_TG_004_VERSION,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

/// Tables exist and start empty; General is unique per chat; deleting a topic cascades to its
/// messages and per-member state. `$seeded` is `Some((chat, owner, message))` after an upgrade.
macro_rules! assert_forum_schema {
    ($pool:expr, $chat:expr, $owner:expr, $old_message:expr) => {{
        let (chat, owner, old_message): (Uuid, Uuid, Option<Uuid>) = ($chat, $owner, $old_message);
        for table in ["forum_topics", "forum_topic_members"] {
            let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one($pool)
                .await
                .unwrap();
            assert_eq!(rows, 0, "{table} starts empty");
        }
        if let Some(old_message) = old_message {
            let topic: Option<Uuid> =
                sqlx::query_scalar("SELECT topic_id FROM messages WHERE id = $1")
                    .bind(old_message)
                    .fetch_one($pool)
                    .await
                    .unwrap();
            assert_eq!(topic, None, "an existing message belongs to General");
        }
        let now = Utc::now();
        let insert_topic = |id: Uuid, general: bool| {
            sqlx::query(
                "INSERT INTO forum_topics (id, room_id, is_general, title, icon_color, created_at) \
                 VALUES ($1, $2, $3, 'topic', 7322096, $4)",
            )
            .bind(id)
            .bind(chat)
            .bind(general)
            .bind(now)
        };
        insert_topic(Uuid::new_v4(), true).execute($pool).await.unwrap();
        assert!(
            insert_topic(Uuid::new_v4(), true).execute($pool).await.is_err(),
            "one General per chat"
        );
        let topic = Uuid::new_v4();
        insert_topic(topic, false).execute($pool).await.unwrap();
        let message = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at, topic_id) \
             VALUES ($1, $2, $3, 'owner', 'in topic', $4, $5)",
        )
        .bind(message)
        .bind(chat)
        .bind(owner)
        .bind(now)
        .bind(topic)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO forum_topic_members (topic_id, user_id, muted, updated_at) \
             VALUES ($1, $2, TRUE, $3)",
        )
        .bind(topic)
        .bind(owner)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        // A topic id from nowhere is refused by the foreign key.
        assert!(sqlx::query(
            "INSERT INTO messages (id, room_id, sender, content, created_at, topic_id) \
             VALUES ($1, $2, 'x', 'x', $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(chat)
        .bind(now)
        .bind(Uuid::new_v4())
        .execute($pool)
        .await
        .is_err());
        sqlx::query("DELETE FROM forum_topics WHERE id = $1")
            .bind(topic)
            .execute($pool)
            .await
            .unwrap();
        for (table, column) in [("messages", "topic_id"), ("forum_topic_members", "topic_id")] {
            let left: i64 =
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE {column} = $1"))
                    .bind(topic)
                    .fetch_one($pool)
                    .await
                    .unwrap();
            assert_eq!(left, 0, "deleting a topic cascades to {table}");
        }
    }};
}

const OLD_MESSAGE: &str = "INSERT INTO messages (id, room_id, sender_id, sender, content, \
    created_at) VALUES ($1, $2, $3, 'owner', 'before the forum', $4)";

/// A chat and its owner through the domain, on a freshly migrated database.
async fn seed_fresh(state: &AppState) -> (Uuid, Uuid) {
    let owner = state.insert_user("tg204-owner", "unused").await.unwrap();
    let chat = Chat {
        id: Uuid::new_v4(),
        title: "tg204-fresh".into(),
        creator_user_id: Some(owner.id),
        join_policy: "open".into(),
        created_at: Utc::now(),
        ..Chat::default()
    };
    state
        .create_chat_with_owner(chat.clone(), owner.id)
        .await
        .unwrap();
    (chat.id, owner.id)
}

async fn full_migrator(directory: &str) -> Migrator {
    Migrator::new(migrations_path(directory)).await.unwrap()
}

#[tokio::test]
async fn sqlite_fresh_and_upgraded_schemas_hold_forum_topics() {
    let database = sqlite_scratch_path("tg204-fresh");
    let state = AppState::open(&database).await.unwrap();
    let (chat, owner) = seed_fresh(&state).await;
    assert_forum_schema!(state.pool(), chat, owner, None);
    state.pool().close().await;
    remove_sqlite_files(&database);

    let database = sqlite_scratch_path("tg204-upgrade");
    let pool = sqlite_pool(&database).await;
    migrations_through(&migrations_path("migrations"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded: SeededChats = seed_sqlite_pre_rename(&pool).await;
    let message = Uuid::new_v4();
    sqlx::query(OLD_MESSAGE)
        .bind(message)
        .bind(seeded.group_chat)
        .bind(seeded.owner)
        .bind(Utc::now())
        .execute(&pool)
        .await
        .unwrap();
    full_migrator("migrations").await.run(&pool).await.unwrap();
    assert_forum_schema!(&pool, seeded.group_chat, seeded.owner, Some(message));
    pool.close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_and_upgraded_schemas_hold_forum_topics() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_and_upgraded_schemas_hold_forum_topics").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg204_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let pool = state.postgres_pool().unwrap();
    let (chat, owner) = seed_fresh(&state).await;
    assert_forum_schema!(pool, chat, owner, None);
    pool.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;

    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg204_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    migrations_through(&migrations_path("migrations-postgres"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_postgres_pre_rename(&pool).await;
    let message = Uuid::new_v4();
    sqlx::query(OLD_MESSAGE)
        .bind(message)
        .bind(seeded.group_chat)
        .bind(seeded.owner)
        .bind(Utc::now())
        .execute(&pool)
        .await
        .unwrap();
    full_migrator("migrations-postgres")
        .await
        .run(&pool)
        .await
        .unwrap();
    assert_forum_schema!(&pool, seeded.group_chat, seeded.owner, Some(message));
    pool.close().await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
