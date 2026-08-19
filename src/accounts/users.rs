//! User and session persistence built on the shared SQLite connection.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::models::{AuthSession, User};
use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
struct UserCredentialRow {
    id: Uuid,
    username: String,
    avatar_emoji: String,
    display_name: String,
    signature: String,
    homepage: String,
    created_at: DateTime<Utc>,
    password_hash: String,
}

impl AppState {
    pub async fn insert_user(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<User, sqlx::Error> {
        let user = User {
            id: Uuid::new_v4(),
            username: username.to_string(),
            avatar_emoji: String::new(),
            display_name: String::new(),
            signature: String::new(),
            homepage: String::new(),
            created_at: Utc::now(),
        };
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO users (id, username, password_hash, avatar_emoji, created_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(user.id)
            .bind(&user.username)
            .bind(password_hash)
            .bind(&user.avatar_emoji)
            .bind(user.created_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(user)
    }

    pub async fn user_credentials(
        &self,
        username: &str,
    ) -> Result<Option<(User, String)>, sqlx::Error> {
        let row: Option<UserCredentialRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, username, avatar_emoji, display_name, signature, homepage, \
                     created_at, password_hash FROM users WHERE LOWER(username) = LOWER($1)",
            )
            .bind(username)
            .fetch_optional(pool)
            .await
        })?;

        Ok(row.map(|row| {
            (
                User {
                    id: row.id,
                    username: row.username,
                    avatar_emoji: row.avatar_emoji,
                    display_name: row.display_name,
                    signature: row.signature,
                    homepage: row.homepage,
                    created_at: row.created_at,
                },
                row.password_hash,
            )
        }))
    }

    pub async fn create_session(&self, user: User) -> Result<AuthSession, sqlx::Error> {
        let token = Uuid::new_v4();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::days(self.session_lifetime_days());
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO sessions (id, user_id, created_at, expires_at) VALUES ($1, $2, $3, $4)",
            )
            .bind(token)
            .bind(user.id)
            .bind(created_at)
            .bind(expires_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;

        Ok(AuthSession {
            token,
            user,
            expires_at,
        })
    }

    pub async fn session_user(&self, token: Uuid) -> Result<Option<User>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, users.homepage, users.created_at FROM sessions \
                 JOIN users ON users.id = sessions.user_id \
                 WHERE sessions.id = $1 AND sessions.expires_at > $2",
            )
            .bind(token)
            .bind(Utc::now())
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn user_by_id(&self, user_id: Uuid) -> Result<Option<User>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, username, avatar_emoji, display_name, signature, homepage, \
                 created_at FROM users WHERE id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn update_user_profile(
        &self,
        user_id: Uuid,
        avatar_emoji: &str,
        display_name: &str,
        signature: &str,
        homepage: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "UPDATE users SET avatar_emoji = $1, display_name = $2, signature = $3, \
                 homepage = $4 WHERE id = $5 RETURNING id, username, avatar_emoji, \
                 display_name, signature, homepage, created_at",
            )
            .bind(avatar_emoji)
            .bind(display_name)
            .bind(signature)
            .bind(homepage)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn update_user_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        current_session: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let changed = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
                .bind(password_hash)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id <> $2")
                .bind(user_id)
                .bind(current_session)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(changed > 0)
        })
    }

    pub async fn delete_user(&self, user_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM users WHERE id = $1")
                .bind(user_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }

    pub async fn delete_session(&self, token: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM sessions WHERE id = $1")
                .bind(token)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }
}
