//! User and session persistence built on the shared SQLite connection.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::models::{AuthSession, User};
use crate::state::{with_pool, AppState};

use super::sessions::SessionMetadata;

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

#[derive(sqlx::FromRow)]
struct SessionUserRow {
    id: Uuid,
    username: String,
    avatar_emoji: String,
    display_name: String,
    signature: String,
    homepage: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl SessionUserRow {
    fn user(&self) -> User {
        User {
            id: self.id,
            username: self.username.clone(),
            avatar_emoji: self.avatar_emoji.clone(),
            display_name: self.display_name.clone(),
            signature: self.signature.clone(),
            homepage: self.homepage.clone(),
            created_at: self.created_at,
        }
    }
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
        self.create_session_with_metadata(user, SessionMetadata::default())
            .await
    }

    pub(crate) async fn create_session_with_metadata(
        &self,
        user: User,
        metadata: SessionMetadata,
    ) -> Result<AuthSession, sqlx::Error> {
        let token = Uuid::new_v4();
        let management_id = Uuid::new_v4().simple().to_string();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::days(self.session_lifetime_days());
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO sessions (id, user_id, management_id, device_name, ip_hint, \
                 created_at, last_used_at, expires_at) VALUES ($1, $2, $3, $4, $5, $6, $6, $7)",
            )
            .bind(token)
            .bind(user.id)
            .bind(management_id)
            .bind(metadata.device_name)
            .bind(metadata.ip_hint)
            .bind(created_at)
            .bind(expires_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;

        let session = AuthSession {
            token,
            user,
            expires_at,
        };
        if let Some(cache) = self.redis_cache() {
            if let Err(error) = cache.set_session(token, &session.user, expires_at).await {
                tracing::warn!("cache newly created session failed: {error:#}");
            }
        }
        Ok(session)
    }

    pub async fn session_user(&self, token: Uuid) -> Result<Option<User>, sqlx::Error> {
        let cached = if let Some(cache) = self.redis_cache() {
            match cache.get_session(token).await {
                Ok(user) => user,
                Err(error) => {
                    tracing::warn!("read Redis session cache failed: {error:#}");
                    None
                }
            }
        } else {
            None
        };
        let now = Utc::now();
        let touched = with_pool!(self, |pool| {
            sqlx::query("UPDATE sessions SET last_used_at = $1 WHERE id = $2 AND expires_at > $1")
                .bind(now)
                .bind(token)
                .execute(pool)
                .await
                .map(|result| result.rows_affected())
        })?;
        if touched == 0 {
            if let (Some(cache), Some(user)) = (self.redis_cache(), cached.as_ref()) {
                if let Err(error) = cache.delete_session(token, user.id).await {
                    tracing::warn!("delete stale Redis session failed: {error:#}");
                }
            }
            return Ok(None);
        }
        if cached.is_some() {
            return Ok(cached);
        }
        let row: Option<SessionUserRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, users.homepage, users.created_at, sessions.expires_at FROM sessions \
                 JOIN users ON users.id = sessions.user_id \
                 WHERE sessions.id = $1 AND sessions.expires_at > $2",
            )
            .bind(token)
            .bind(now)
            .fetch_optional(pool)
            .await
        })?;
        if let (Some(cache), Some(row)) = (self.redis_cache(), row.as_ref()) {
            if let Err(error) = cache.set_session(token, &row.user(), row.expires_at).await {
                tracing::warn!("populate Redis session cache failed: {error:#}");
            }
        }
        Ok(row.map(|row| row.user()))
    }

    pub(crate) async fn session_active(&self, token: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1 AND expires_at > $2)",
            )
            .bind(token)
            .bind(Utc::now())
            .fetch_one(pool)
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
        let updated = with_pool!(self, |pool| {
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
        })?;
        if updated.is_some() {
            self.invalidate_user_sessions(user_id).await;
        }
        Ok(updated)
    }

    pub async fn update_user_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        current_session: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
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
        })?;
        if changed {
            self.invalidate_user_sessions(user_id).await;
        }
        Ok(changed)
    }

    pub async fn delete_user(&self, user_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
        let room_ids = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let direct_room_ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT room_id FROM direct_conversations \
                 WHERE user_low_id = $1 OR user_high_id = $1",
            )
            .bind(user_id)
            .fetch_all(&mut *transaction)
            .await?;
            for room_id in &direct_room_ids {
                sqlx::query(
                    "UPDATE rooms SET deleted_at = $1 WHERE id = $2 AND deleted_at IS NULL",
                )
                .bind(Utc::now())
                .bind(room_id)
                .execute(&mut *transaction)
                .await?;
            }
            let removed = sqlx::query("DELETE FROM users WHERE id = $1")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(if removed > 0 {
                direct_room_ids
            } else {
                Vec::new()
            })
        })?;
        self.invalidate_user_sessions(user_id).await;
        Ok(room_ids)
    }

    pub async fn delete_session(&self, token: Uuid) -> Result<bool, sqlx::Error> {
        let user_id: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("DELETE FROM sessions WHERE id = $1 RETURNING user_id")
                .bind(token)
                .fetch_optional(pool)
                .await
        })?;
        if let (Some(cache), Some(user_id)) = (self.redis_cache(), user_id) {
            if let Err(error) = cache.delete_session(token, user_id).await {
                tracing::warn!("delete Redis session cache failed: {error:#}");
            }
        }
        Ok(user_id.is_some())
    }

    pub(crate) async fn invalidate_user_sessions(&self, user_id: Uuid) {
        if let Some(cache) = self.redis_cache() {
            if let Err(error) = cache.delete_user_sessions(user_id).await {
                tracing::warn!("invalidate Redis user sessions failed: {error:#}");
            }
        }
    }
}
