//! What migration `20261001000003` must have done to the rows, and proof that the renamed
//! foreign keys still cascade for real rather than merely parsing.

#![allow(dead_code)]

use sqlx::{PgPool, SqlitePool};

use super::chat_schema::SeededChats;

macro_rules! assert_backfill {
    ($pool:expr, $seeded:expr) => {{
        let seeded = $seeded;
        let (chat_type, title, access_hash, member_count): (String, String, String, i64) =
            sqlx::query_as(
                "SELECT chat_type, title, access_hash, CAST(member_count AS BIGINT) \
                 FROM chats WHERE id = $1",
            )
            .bind(seeded.group_chat)
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(chat_type, "group", "a chat outside direct_conversations");
        assert_eq!(title, "tg004-group", "rooms.name became chats.title");
        assert_eq!(access_hash.len(), 16, "64 bits of access hash, hex encoded");
        assert_eq!(member_count, 2, "two active members were counted");

        let (chat_type, access_hash, member_count): (String, String, i64) = sqlx::query_as(
            "SELECT chat_type, access_hash, CAST(member_count AS BIGINT) FROM chats WHERE id = $1",
        )
        .bind(seeded.direct_chat)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(
            chat_type, "private",
            "a chat in direct_conversations is a private chat"
        );
        assert_eq!(member_count, 1);
        assert_ne!(
            access_hash, "",
            "every pre-existing chat gets an access hash"
        );

        // Distinct per row: a shared constant would defeat the point of the column.
        let distinct: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT access_hash) FROM chats WHERE id IN ($1, $2)",
        )
        .bind(seeded.group_chat)
        .bind(seeded.direct_chat)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(distinct, 2, "access hashes are generated per chat");

        let defaults: (i64, i64, i64) = sqlx::query_as(
            "SELECT CAST(slow_mode_seconds AS BIGINT), CAST(auto_delete_seconds AS BIGINT), \
             CAST(member_count AS BIGINT) FROM chats WHERE id = $1",
        )
        .bind(seeded.group_chat)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(defaults.0, 0);
        assert_eq!(defaults.1, 0);
        assert_eq!(defaults.2, 2);
    }};
}

/// Deletes the seeded group chat and checks that the dependent rows went with it. This is the
/// behavioural half of the foreign-key test: a `REFERENCES` clause that parses but points at
/// the wrong table would still pass a schema inspection, and would not pass this.
macro_rules! assert_cascade {
    ($pool:expr, $seeded:expr) => {{
        let seeded = $seeded;
        let before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chat_members WHERE room_id = $1")
                .bind(seeded.group_chat)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(before, 2);
        sqlx::query("DELETE FROM chats WHERE id = $1")
            .bind(seeded.group_chat)
            .execute($pool)
            .await
            .unwrap();
        for table in ["chat_members", "chat_roles"] {
            let remaining: i64 =
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE room_id = $1"))
                    .bind(seeded.group_chat)
                    .fetch_one($pool)
                    .await
                    .unwrap();
            assert_eq!(
                remaining, 0,
                "{table}.room_id no longer cascades from chats(id)"
            );
        }
        // The direct chat is untouched: the cascade is scoped, not a blanket delete.
        let survivors: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chat_members WHERE room_id = $1")
                .bind(seeded.direct_chat)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(survivors, 1);
    }};
}

pub async fn assert_sqlite_backfill(pool: &SqlitePool, seeded: &SeededChats) {
    assert_backfill!(pool, seeded)
}

pub async fn assert_postgres_backfill(pool: &PgPool, seeded: &SeededChats) {
    assert_backfill!(pool, seeded)
}

pub async fn assert_sqlite_cascade(pool: &SqlitePool, seeded: &SeededChats) {
    assert_cascade!(pool, seeded)
}

pub async fn assert_postgres_cascade(pool: &PgPool, seeded: &SeededChats) {
    assert_cascade!(pool, seeded)
}
