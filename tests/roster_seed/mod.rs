//! Bulk roster seeding for the supergroup and pagination tests: set-based SQL, one statement
//! for the accounts and one for the memberships, so 200 000 members take seconds rather than
//! 200 000 round trips. Memberships go through the real `chat_members` table, so the
//! member_count triggers run exactly as they do in production.

#![allow(dead_code)]

use chat_room::state::AppState;
use chrono::Utc;
use uuid::Uuid;

/// First second of 2026; seeded members join one second apart per pair, so pages contain
/// `joined_at` ties that the `user_id` tie-break must resolve.
pub const SEED_EPOCH: i64 = 1_767_225_600;

/// Create `count` accounts named `<prefix><n>` and make them active members of `room_id`.
pub async fn seed_members(state: &AppState, room_id: Uuid, prefix: &str, count: i64) {
    let role_id = format!("{}:member", room_id.simple());
    let now = Utc::now();
    let pattern = format!("{prefix}%");
    if let Some(pool) = state.postgres_pool() {
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) \
             SELECT gen_random_uuid(), $1 || n, 'x', $2 FROM generate_series(1, $3) AS n",
        )
        .bind(prefix)
        .bind(now)
        .bind(count)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chat_members (room_id, user_id, role_id, status, requested_at, joined_at) \
             SELECT $1, id, $2, 'active', $3, \
               to_timestamp($4 + (row_number() OVER (ORDER BY username)) / 2) \
             FROM users WHERE username LIKE $5",
        )
        .bind(room_id)
        .bind(&role_id)
        .bind(now)
        .bind(SEED_EPOCH)
        .bind(&pattern)
        .execute(pool)
        .await
        .unwrap();
        // What autovacuum does shortly after a bulk load in production. Without statistics the
        // planner still believes chat_members is tiny and sorts the whole roster once.
        sqlx::query("ANALYZE users, chat_members")
            .execute(pool)
            .await
            .unwrap();
    } else {
        let pool = state.pool();
        sqlx::query(
            "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < $1) \
             INSERT INTO users (id, username, password_hash, created_at) \
             SELECT randomblob(16), $2 || n, 'x', $3 FROM seq",
        )
        .bind(count)
        .bind(prefix)
        .bind(now)
        .execute(pool)
        .await
        .unwrap();
        // `joined_at` is written in the exact text form sqlx gives a whole-second
        // DateTime<Utc>, so seeded rows compare like rows the server writes.
        sqlx::query(
            "INSERT INTO chat_members (room_id, user_id, role_id, status, requested_at, joined_at) \
             SELECT $1, id, $2, 'active', $3, \
               strftime('%Y-%m-%dT%H:%M:%S+00:00', $4 + rowid / 2, 'unixepoch') \
             FROM users WHERE username LIKE $5",
        )
        .bind(room_id)
        .bind(&role_id)
        .bind(now)
        .bind(SEED_EPOCH)
        .bind(&pattern)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// Insert one account directly (no password hashing) and return its id.
pub async fn insert_account(state: &AppState, username: &str) -> Uuid {
    let id = Uuid::new_v4();
    let query = "INSERT INTO users (id, username, password_hash, created_at) \
                 VALUES ($1, $2, 'x', $3)";
    if let Some(pool) = state.postgres_pool() {
        sqlx::query(query)
            .bind(id)
            .bind(username)
            .bind(Utc::now())
            .execute(pool)
            .await
            .unwrap();
    } else {
        sqlx::query(query)
            .bind(id)
            .bind(username)
            .bind(Utc::now())
            .execute(state.pool())
            .await
            .unwrap();
    }
    id
}
