//! TG-205: migration `20261101000006` (chat_invite_links + chat_members.invite_link_id) on
//! both adapters, on a fresh schema and upgraded from the pre-TG-004 baseline with seeded
//! chats: the table starts empty, existing memberships keep a NULL link, one live primary
//! per chat is enforced, a limit and an approval flag exclude each other, and deleting a link
//! leaves the members it admitted in place.

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

macro_rules! insert_link {
    ($pool:expr, $room:expr, $primary:expr, $limit:expr, $approval:expr) => {
        sqlx::query(
            "INSERT INTO chat_invite_links (id, room_id, token, is_primary, usage_limit, \
             requires_approval, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::new_v4())
        .bind($room)
        .bind(Uuid::new_v4().simple().to_string())
        .bind($primary)
        .bind($limit)
        .bind($approval)
        .bind(Utc::now())
        .execute($pool)
        .await
    };
}

macro_rules! assert_invite_link_schema {
    ($pool:expr, $room:expr) => {{
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_invite_links")
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "chat_invite_links starts empty");
        let linked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM chat_members WHERE invite_link_id IS NOT NULL",
        )
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(linked, 0, "existing memberships came through no link");

        insert_link!($pool, $room, true, None::<i64>, false).unwrap();
        assert!(
            insert_link!($pool, $room, true, None::<i64>, false).is_err(),
            "a second live primary link is refused"
        );
        assert!(
            insert_link!($pool, $room, false, Some(5_i64), true).is_err(),
            "a limit and an approval flag exclude each other"
        );
        assert!(insert_link!($pool, $room, false, Some(0_i64), false).is_err());
        sqlx::query("UPDATE chat_invite_links SET revoked_at = $1 WHERE is_primary")
            .bind(Utc::now())
            .execute($pool)
            .await
            .unwrap();
        insert_link!($pool, $room, true, None::<i64>, false).unwrap();

        // A member admitted by a link survives the link's deletion.
        let link: Uuid = sqlx::query_scalar(
            "SELECT id FROM chat_invite_links WHERE is_primary AND revoked_at IS NULL",
        )
        .fetch_one($pool)
        .await
        .unwrap();
        let member: Uuid =
            sqlx::query_scalar("SELECT user_id FROM chat_members WHERE room_id = $1 LIMIT 1")
                .bind($room)
                .fetch_one($pool)
                .await
                .unwrap();
        sqlx::query(
            "UPDATE chat_members SET invite_link_id = $1 WHERE room_id = $2 AND user_id = $3",
        )
        .bind(link)
        .bind($room)
        .bind(member)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM chat_invite_links WHERE id = $1")
            .bind(link)
            .execute($pool)
            .await
            .unwrap();
        let kept: Option<Option<Uuid>> = sqlx::query_scalar(
            "SELECT invite_link_id FROM chat_members WHERE room_id = $1 AND user_id = $2",
        )
        .bind($room)
        .bind(member)
        .fetch_optional($pool)
        .await
        .unwrap();
        assert_eq!(kept, Some(None), "membership kept, link forgotten");
        // Deleting the chat takes its links with it.
        sqlx::query("DELETE FROM chats WHERE id = $1")
            .bind($room)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_invite_links")
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(left, 0);
    }};
}

async fn a_chat_with_a_member(state: &AppState) -> Uuid {
    let user = state.register_user("tg205-mig", "x", None).await.unwrap();
    let chat = Chat {
        id: Uuid::new_v4(),
        title: "迁移".into(),
        creator_user_id: Some(user.id),
        join_policy: "open".into(),
        created_at: Utc::now(),
        ..Chat::default()
    };
    state
        .create_chat_with_owner(chat.clone(), user.id)
        .await
        .unwrap();
    chat.id
}

#[tokio::test]
async fn sqlite_fresh_schema_has_invite_links() {
    let database = sqlite_scratch_path("tg205-fresh");
    let state = AppState::open(&database).await.unwrap();
    let room = a_chat_with_a_member(&state).await;
    assert_invite_link_schema!(state.pool(), room);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_adds_invite_links() {
    let database = sqlite_scratch_path("tg205-upgrade");
    let pool = sqlite_pool(&database).await;
    migrations_through(&migrations_path("migrations"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded: SeededChats = seed_sqlite_pre_rename(&pool).await;
    Migrator::new(migrations_path("migrations"))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    assert_invite_link_schema!(&pool, seeded.group_chat);
    pool.close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_invite_links() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_invite_links").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg205_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let room = a_chat_with_a_member(&state).await;
    assert_invite_link_schema!(state.postgres_pool().unwrap(), room);
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_adds_invite_links() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_adds_invite_links").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg205_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    migrations_through(&migrations_path("migrations-postgres"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_postgres_pre_rename(&pool).await;
    Migrator::new(migrations_path("migrations-postgres"))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    assert_invite_link_schema!(&pool, seeded.group_chat);
    pool.close().await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
