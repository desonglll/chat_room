//! TG-506 persistence on `AppState`: credentials, pending login challenges, mailed codes.
//!
//! Every attempt counter is reserved atomically (`UPDATE … attempts < max RETURNING`)
//! *before* the expensive argon2 check, so parallel guesses can never exceed the cap.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
pub(super) struct CredentialRow {
    pub password_hash: String,
    pub hint: String,
    pub recovery_email: Option<String>,
}

#[derive(sqlx::FromRow)]
pub(super) struct CodeRow {
    pub email: String,
    pub code_hash: String,
}

#[derive(Clone, Copy)]
pub(super) enum CodePurpose {
    VerifyEmail,
    Reset,
}

impl CodePurpose {
    fn key(self) -> &'static str {
        match self {
            Self::VerifyEmail => "verify_email",
            Self::Reset => "reset",
        }
    }
}

pub(super) fn token_hash(token: Uuid) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

impl AppState {
    pub(super) async fn two_factor_credential(
        &self,
        user_id: Uuid,
    ) -> Result<Option<CredentialRow>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT password_hash, hint, recovery_email FROM two_factor_credentials \
                 WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    /// Turn 2FA on and, in the same transaction, end every other session (D-010).
    /// Returns false when 2FA was already on (nothing changes then).
    pub(super) async fn enable_two_factor(
        &self,
        user_id: Uuid,
        password_hash: &str,
        hint: &str,
        keep_session: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let enabled = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let inserted = sqlx::query(
                "INSERT INTO two_factor_credentials \
                 (user_id, password_hash, hint, recovery_email, created_at, updated_at) \
                 VALUES ($1, $2, $3, NULL, $4, $4) ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(password_hash)
            .bind(hint)
            .bind(now)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if inserted > 0 {
                sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id <> $2")
                    .bind(user_id)
                    .bind(keep_session)
                    .execute(&mut *transaction)
                    .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(inserted > 0)
        })?;
        if enabled {
            self.invalidate_user_sessions(user_id).await;
        }
        Ok(enabled)
    }

    pub(super) async fn update_two_factor(
        &self,
        user_id: Uuid,
        password_hash: Option<&str>,
        hint: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE two_factor_credentials SET \
                 password_hash = COALESCE($1, password_hash), hint = $2, updated_at = $3 \
                 WHERE user_id = $4",
            )
            .bind(password_hash)
            .bind(hint)
            .bind(Utc::now())
            .bind(user_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    /// Remove 2FA with its codes and pending challenges. With `end_sessions_except`
    /// (the recovery reset) every session of the account except that one also ends.
    pub(super) async fn remove_two_factor(
        &self,
        user_id: Uuid,
        end_sessions_except: Option<Uuid>,
    ) -> Result<bool, sqlx::Error> {
        let removed = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let removed = sqlx::query("DELETE FROM two_factor_credentials WHERE user_id = $1")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            for table in ["two_factor_email_codes", "two_factor_login_challenges"] {
                sqlx::query(&format!("DELETE FROM {table} WHERE user_id = $1"))
                    .bind(user_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            if let Some(keep) = end_sessions_except {
                sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id <> $2")
                    .bind(user_id)
                    .bind(keep)
                    .execute(&mut *transaction)
                    .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(removed > 0)
        })?;
        if end_sessions_except.is_some() {
            self.invalidate_user_sessions(user_id).await;
        }
        Ok(removed)
    }

    pub(super) async fn set_recovery_email(
        &self,
        user_id: Uuid,
        email: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE two_factor_credentials SET recovery_email = $1, updated_at = $2 \
                 WHERE user_id = $3",
            )
            .bind(email)
            .bind(Utc::now())
            .bind(user_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    pub(super) async fn create_login_challenge(
        &self,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Result<Uuid, sqlx::Error> {
        let token = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            // Opportunistic sweep so abandoned challenges do not accumulate.
            sqlx::query("DELETE FROM two_factor_login_challenges WHERE expires_at <= $1")
                .bind(now)
                .execute(pool)
                .await?;
            sqlx::query(
                "INSERT INTO two_factor_login_challenges \
                 (token_hash, user_id, attempts, expires_at, created_at) VALUES ($1, $2, 0, $3, $4)",
            )
            .bind(token_hash(token))
            .bind(user_id)
            .bind(expires_at)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(token)
    }

    /// The account behind a live challenge, without spending an attempt.
    pub(super) async fn login_challenge_user(
        &self,
        token: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT user_id FROM two_factor_login_challenges \
                 WHERE token_hash = $1 AND expires_at > $2",
            )
            .bind(token_hash(token))
            .bind(Utc::now())
            .fetch_optional(pool)
            .await
        })
    }

    /// Spend one attempt on a live challenge; `None` once expired or exhausted.
    pub(super) async fn reserve_login_attempt(
        &self,
        token: Uuid,
        max_attempts: i32,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "UPDATE two_factor_login_challenges SET attempts = attempts + 1 \
                 WHERE token_hash = $1 AND expires_at > $2 AND attempts < $3 RETURNING user_id",
            )
            .bind(token_hash(token))
            .bind(Utc::now())
            .bind(max_attempts)
            .fetch_optional(pool)
            .await
        })
    }

    /// Consume a challenge exactly once; a racing second request gets `None`.
    pub(super) async fn consume_login_challenge(
        &self,
        token: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "DELETE FROM two_factor_login_challenges \
                 WHERE token_hash = $1 AND expires_at > $2 RETURNING user_id",
            )
            .bind(token_hash(token))
            .bind(Utc::now())
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn store_email_code(
        &self,
        user_id: Uuid,
        purpose: CodePurpose,
        email: &str,
        code_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO two_factor_email_codes \
                 (user_id, purpose, email, code_hash, attempts, expires_at, created_at) \
                 VALUES ($1, $2, $3, $4, 0, $5, $6) ON CONFLICT (user_id, purpose) DO UPDATE SET \
                 email = excluded.email, code_hash = excluded.code_hash, attempts = 0, \
                 expires_at = excluded.expires_at, created_at = excluded.created_at",
            )
            .bind(user_id)
            .bind(purpose.key())
            .bind(email)
            .bind(code_hash)
            .bind(expires_at)
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    /// The address a live, unconfirmed code was sent to.
    pub(super) async fn pending_code_email(
        &self,
        user_id: Uuid,
        purpose: CodePurpose,
    ) -> Result<Option<String>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT email FROM two_factor_email_codes \
                 WHERE user_id = $1 AND purpose = $2 AND expires_at > $3",
            )
            .bind(user_id)
            .bind(purpose.key())
            .bind(Utc::now())
            .fetch_optional(pool)
            .await
        })
    }

    /// Spend one attempt on a live code and return it for verification.
    pub(super) async fn reserve_code_attempt(
        &self,
        user_id: Uuid,
        purpose: CodePurpose,
        max_attempts: i32,
    ) -> Result<Option<CodeRow>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "UPDATE two_factor_email_codes SET attempts = attempts + 1 \
                 WHERE user_id = $1 AND purpose = $2 AND expires_at > $3 AND attempts < $4 \
                 RETURNING email, code_hash",
            )
            .bind(user_id)
            .bind(purpose.key())
            .bind(Utc::now())
            .bind(max_attempts)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn delete_email_code(
        &self,
        user_id: Uuid,
        purpose: CodePurpose,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM two_factor_email_codes WHERE user_id = $1 AND purpose = $2")
                .bind(user_id)
                .bind(purpose.key())
                .execute(pool)
                .await
                .map(|_| ())
        })
    }
}
