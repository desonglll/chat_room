use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

use super::{models::SystemAdminRow, token_hash, AdminRoleError, SystemAdminView};

macro_rules! append_event {
    ($transaction:expr, $actor_id:expr, $subject_id:expr, $action:expr, $created_at:expr) => {
        sqlx::query(
            "INSERT INTO system_admin_events \
             (id, actor_user_id, subject_user_id, action, created_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind($actor_id)
        .bind($subject_id)
        .bind($action)
        .bind($created_at)
        .execute(&mut *$transaction)
        .await?;
    };
}

macro_rules! lock_roles {
    ($transaction:expr) => {
        sqlx::query(
            "UPDATE system_settings SET value = value \
             WHERE key = 'system_admin_bootstrap_completed'",
        )
        .execute(&mut *$transaction)
        .await?;
    };
}

macro_rules! actor_is_admin {
    ($transaction:expr, $actor_id:expr) => {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM system_admins WHERE user_id = $1)")
            .bind($actor_id)
            .fetch_one(&mut *$transaction)
            .await?
    };
}

impl AppState {
    pub async fn is_system_admin(&self, user_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM system_admins WHERE user_id = $1)")
                .bind(user_id)
                .fetch_one(pool)
                .await
        })
    }

    pub async fn list_system_admins(&self) -> Result<Vec<SystemAdminView>, sqlx::Error> {
        let rows: Vec<SystemAdminRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, users.homepage, users.created_at AS user_created_at, \
                 system_admins.granted_by, system_admins.grant_source, \
                 system_admins.created_at AS admin_created_at \
                 FROM system_admins JOIN users ON users.id = system_admins.user_id \
                 ORDER BY system_admins.created_at, users.id",
            )
            .fetch_all(pool)
            .await
        })?;
        Ok(rows.into_iter().map(SystemAdminRow::view).collect())
    }

    pub async fn import_legacy_system_admins(
        &self,
        usernames: &[String],
    ) -> Result<u64, sqlx::Error> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let marker: String = sqlx::query_scalar(
                "SELECT value FROM system_settings \
                 WHERE key = 'legacy_admin_usernames_migrated'",
            )
            .fetch_one(&mut *transaction)
            .await?;
            if marker == "true" {
                transaction.commit().await?;
                return Ok(0);
            }

            let now = Utc::now();
            let mut imported = 0;
            for username in usernames {
                let user_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM users WHERE LOWER(username) = LOWER($1)")
                        .bind(username)
                        .fetch_optional(&mut *transaction)
                        .await?;
                let Some(user_id) = user_id else { continue };
                let inserted = sqlx::query(
                    "INSERT INTO system_admins \
                     (user_id, granted_by, grant_source, created_at) \
                     VALUES ($1, NULL, 'legacy_config', $2) \
                     ON CONFLICT(user_id) DO NOTHING",
                )
                .bind(user_id)
                .bind(now)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
                if inserted > 0 {
                    append_event!(transaction, None::<Uuid>, user_id, "legacy_import", now);
                    imported += 1;
                }
            }
            sqlx::query(
                "UPDATE system_settings SET value = 'true' \
                 WHERE key = 'legacy_admin_usernames_migrated'",
            )
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok(imported)
        })
    }

    pub async fn bootstrap_system_admin(
        &self,
        username: &str,
    ) -> Result<SystemAdminView, AdminRoleError> {
        let user_id = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE system_settings SET value = value \
                 WHERE key = 'system_admin_bootstrap_completed'",
            )
            .execute(&mut *transaction)
            .await?;
            let completed: String = sqlx::query_scalar(
                "SELECT value FROM system_settings \
                 WHERE key = 'system_admin_bootstrap_completed'",
            )
            .fetch_one(&mut *transaction)
            .await?;
            let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_admins")
                .fetch_one(&mut *transaction)
                .await?;
            if completed == "true" || admin_count > 0 {
                return Err(AdminRoleError::BootstrapUnavailable);
            }
            let user_id: Uuid =
                sqlx::query_scalar("SELECT id FROM users WHERE LOWER(username) = LOWER($1)")
                    .bind(username)
                    .fetch_optional(&mut *transaction)
                    .await?
                    .ok_or(AdminRoleError::UserNotFound)?;
            let now = Utc::now();
            sqlx::query(
                "INSERT INTO system_admins \
                 (user_id, granted_by, grant_source, created_at) \
                 VALUES ($1, NULL, 'bootstrap', $2)",
            )
            .bind(user_id)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            append_event!(transaction, None::<Uuid>, user_id, "bootstrap", now);
            sqlx::query(
                "UPDATE system_settings SET value = 'true' \
                 WHERE key = 'system_admin_bootstrap_completed'",
            )
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, AdminRoleError>(user_id)
        })?;
        self.system_admin(user_id)
            .await?
            .ok_or(AdminRoleError::UserNotFound)
    }

    pub async fn grant_system_admin(
        &self,
        actor_id: Uuid,
        user_id: Uuid,
    ) -> Result<SystemAdminView, AdminRoleError> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            lock_roles!(transaction);
            if !actor_is_admin!(transaction, actor_id) {
                return Err(AdminRoleError::Forbidden);
            }
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
                    .bind(user_id)
                    .fetch_one(&mut *transaction)
                    .await?;
            if !exists {
                return Err(AdminRoleError::UserNotFound);
            }
            let now = Utc::now();
            let inserted = sqlx::query(
                "INSERT INTO system_admins \
                 (user_id, granted_by, grant_source, created_at) \
                 VALUES ($1, $2, 'administrator', $3) \
                 ON CONFLICT(user_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(actor_id)
            .bind(now)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if inserted > 0 {
                append_event!(transaction, Some(actor_id), user_id, "grant", now);
            }
            transaction.commit().await?;
            Ok::<_, AdminRoleError>(())
        })?;
        self.system_admin(user_id)
            .await?
            .ok_or(AdminRoleError::UserNotFound)
    }

    pub async fn revoke_system_admin(
        &self,
        actor_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AdminRoleError> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            lock_roles!(transaction);
            if !actor_is_admin!(transaction, actor_id) {
                return Err(AdminRoleError::Forbidden);
            }
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM system_admins")
                .fetch_one(&mut *transaction)
                .await?;
            let target_exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM system_admins WHERE user_id = $1)")
                    .bind(user_id)
                    .fetch_one(&mut *transaction)
                    .await?;
            if !target_exists {
                return Err(AdminRoleError::AdministratorNotFound);
            }
            if count <= 1 {
                return Err(AdminRoleError::LastAdministrator);
            }
            sqlx::query("DELETE FROM system_admins WHERE user_id = $1")
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            append_event!(transaction, Some(actor_id), user_id, "revoke", Utc::now());
            transaction.commit().await?;
            Ok::<_, AdminRoleError>(())
        })
    }

    pub async fn create_registration_invite(
        &self,
        actor_id: Uuid,
        lifetime_hours: i64,
    ) -> Result<(String, DateTime<Utc>), AdminRoleError> {
        let token = format!("egi_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let hash = token_hash(&token);
        let created_at = Utc::now();
        let expires_at = created_at + Duration::hours(lifetime_hours);
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            lock_roles!(transaction);
            if !actor_is_admin!(transaction, actor_id) {
                return Err(AdminRoleError::Forbidden);
            }
            sqlx::query(
                "INSERT INTO registration_invites \
                 (token_hash, created_by, created_at, expires_at) VALUES ($1, $2, $3, $4)",
            )
            .bind(hash)
            .bind(actor_id)
            .bind(created_at)
            .bind(expires_at)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, AdminRoleError>(())
        })?;
        Ok((token, expires_at))
    }

    async fn system_admin(&self, user_id: Uuid) -> Result<Option<SystemAdminView>, sqlx::Error> {
        let row: Option<SystemAdminRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, users.homepage, users.created_at AS user_created_at, \
                 system_admins.granted_by, system_admins.grant_source, \
                 system_admins.created_at AS admin_created_at \
                 FROM system_admins JOIN users ON users.id = system_admins.user_id \
                 WHERE system_admins.user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.map(SystemAdminRow::view))
    }
}
