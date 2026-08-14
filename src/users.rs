//! User and session persistence built on the shared SQLite connection.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::models::{AuthSession, User};
use crate::state::AppState;

const SESSION_LIFETIME_DAYS: i64 = 30;

impl AppState {
    pub async fn insert_user(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<User, sqlx::Error> {
        let user = User {
            id: Uuid::new_v4(),
            username: username.to_string(),
            created_at: Utc::now(),
        };
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(user.id)
        .bind(&user.username)
        .bind(password_hash)
        .bind(user.created_at)
        .execute(self.pool())
        .await?;
        Ok(user)
    }

    pub async fn user_credentials(
        &self,
        username: &str,
    ) -> Result<Option<(User, String)>, sqlx::Error> {
        let row: Option<(Uuid, String, DateTime<Utc>, String)> = sqlx::query_as(
            "SELECT id, username, created_at, password_hash FROM users \
             WHERE username = ? COLLATE NOCASE",
        )
        .bind(username)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(id, username, created_at, password_hash)| {
            (
                User {
                    id,
                    username,
                    created_at,
                },
                password_hash,
            )
        }))
    }

    pub async fn create_session(&self, user: User) -> Result<AuthSession, sqlx::Error> {
        let token = Uuid::new_v4();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::days(SESSION_LIFETIME_DAYS);
        sqlx::query(
            "INSERT INTO sessions (id, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(token)
        .bind(user.id)
        .bind(created_at)
        .bind(expires_at)
        .execute(self.pool())
        .await?;

        Ok(AuthSession {
            token,
            user,
            expires_at,
        })
    }

    pub async fn session_user(&self, token: Uuid) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as(
            "SELECT users.id, users.username, users.created_at FROM sessions \
             JOIN users ON users.id = sessions.user_id \
             WHERE sessions.id = ? AND sessions.expires_at > ?",
        )
        .bind(token)
        .bind(Utc::now())
        .fetch_optional(self.pool())
        .await
    }

    pub async fn delete_session(&self, token: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(token)
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
