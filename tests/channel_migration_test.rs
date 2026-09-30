//! TG-202: migration `20261101000003` (views, signatures, `message_views`) on both adapters,
//! on a fresh schema and upgraded from `20261101000002` with a channel and a post already in
//! place.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, migrations_path, migrations_through,
    postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool, sqlite_scratch_path,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

const BEFORE_TG_202: i64 = 20261101000002;

struct Seeded {
    author: Uuid,
    channel: Uuid,
    group: Uuid,
    old_post: Uuid,
}

/// A user, a signed channel, a group and one channel post, with plain SQL that is valid on
/// both adapters at `20261101000002`.
macro_rules! seed {
    ($pool:expr) => {{
        let now = Utc::now();
        let seeded = Seeded {
            author: Uuid::new_v4(),
            channel: Uuid::new_v4(),
            group: Uuid::new_v4(),
            old_post: Uuid::new_v4(),
        };
        sqlx::query(
            "INSERT INTO users (id, username, display_name, password_hash, created_at) \
             VALUES ($1, 'tg202-author', 'Author Name', 'x', $2)",
        )
        .bind(seeded.author)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        for (id, chat_type, title) in [
            (seeded.channel, "channel", "tg202-channel"),
            (seeded.group, "group", "tg202-group"),
        ] {
            sqlx::query(
                "INSERT INTO chats (id, chat_type, title, password_hash, join_policy, \
                 access_hash, signatures_enabled, created_at) \
                 VALUES ($1, $2, $3, '', 'open', $4, $5, $6)",
            )
            .bind(id)
            .bind(chat_type)
            .bind(title)
            .bind(format!("{:016x}", id.as_u128() as u64))
            .bind(true)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        insert_message!($pool, seeded.old_post, seeded.channel, seeded.author);
        seeded
    }};
}

macro_rules! insert_message {
    ($pool:expr, $id:expr, $room:expr, $sender:expr) => {
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES ($1, $2, $3, 'sender', 'body', $4)",
        )
        .bind($id)
        .bind($room)
        .bind($sender)
        .bind(Utc::now())
        .execute($pool)
        .await
        .unwrap()
    };
}

macro_rules! post_fields {
    ($pool:expr, $id:expr) => {
        sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT CAST(views_count AS BIGINT), post_author FROM messages WHERE id = $1",
        )
        .bind($id)
        .fetch_one($pool)
        .await
        .unwrap()
    };
}

/// The post-migration behaviour: signatures on new channel posts only, views rows keyed by
/// (post, viewer) and cascading with the post.
macro_rules! assert_channel_posts {
    ($pool:expr, $seeded:expr, $old_author:expr) => {{
        let seeded: &Seeded = $seeded;
        let old_author: Option<&str> = $old_author;
        assert_eq!(
            post_fields!($pool, seeded.old_post),
            (0, old_author.map(str::to_string)),
            "a post written before the migration is unsigned with 0 views"
        );
        let new_post = Uuid::new_v4();
        insert_message!($pool, new_post, seeded.channel, seeded.author);
        assert_eq!(
            post_fields!($pool, new_post),
            (0, Some("Author Name".to_string())),
            "the trigger signs a post of a signed channel"
        );
        let group_message = Uuid::new_v4();
        insert_message!($pool, group_message, seeded.group, seeded.author);
        assert_eq!(
            post_fields!($pool, group_message),
            (0, None),
            "groups are never signed"
        );

        let now = Utc::now();
        let insert_view = |message: Uuid| {
            sqlx::query(
                "INSERT INTO message_views (message_id, user_id, viewed_at) VALUES ($1, $2, $3) \
                 ON CONFLICT (message_id, user_id) DO NOTHING",
            )
            .bind(message)
            .bind(seeded.author)
            .bind(now)
        };
        assert_eq!(
            insert_view(new_post)
                .execute($pool)
                .await
                .unwrap()
                .rows_affected(),
            1
        );
        assert_eq!(
            insert_view(new_post)
                .execute($pool)
                .await
                .unwrap()
                .rows_affected(),
            0
        );
        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(new_post)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message_views")
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(left, 0, "views cascade with their post");
    }};
}

async fn full_migrator(directory: &str) -> Migrator {
    Migrator::new(migrations_path(directory)).await.unwrap()
}

#[tokio::test]
async fn sqlite_fresh_schema_signs_and_counts_channel_posts() {
    let database = sqlite_scratch_path("tg202-fresh");
    let state = AppState::open(&database).await.unwrap();
    let seeded = seed!(state.pool());
    assert_channel_posts!(state.pool(), &seeded, Some("Author Name"));
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_keeps_old_posts_unsigned() {
    let database = sqlite_scratch_path("tg202-upgrade");
    let pool = sqlite_pool(&database).await;
    migrations_through(&migrations_path("migrations"), BEFORE_TG_202)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed!(&pool);
    full_migrator("migrations").await.run(&pool).await.unwrap();
    assert_channel_posts!(&pool, &seeded, None);
    pool.close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_signs_and_counts_channel_posts() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_signs_and_counts_channel_posts").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg202_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let pool = state.postgres_pool().unwrap();
    let seeded = seed!(pool);
    assert_channel_posts!(pool, &seeded, Some("Author Name"));
    pool.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_keeps_old_posts_unsigned() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_keeps_old_posts_unsigned").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg202_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    migrations_through(&migrations_path("migrations-postgres"), BEFORE_TG_202)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed!(&pool);
    full_migrator("migrations-postgres")
        .await
        .run(&pool)
        .await
        .unwrap();
    assert_channel_posts!(&pool, &seeded, None);
    pool.close().await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
