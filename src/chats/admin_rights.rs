//! Appointing and dismissing administrators with an explicit set of rights and a title.
//!
//! An administrator holds the shared `admin` role. When they were appointed with a selection,
//! `chat_admin_rights` carries exactly their rights (always including
//! `permissions::ADMIN_BASELINE`) and replaces the role's grants for them in step 3 of the
//! decision. Dismissal moves them back to the `member` role and deletes the rows.

use uuid::Uuid;

use super::permissions::ADMIN_BASELINE;
use crate::state::{with_pool, AppState};

/// Telegram's limit for an admin title.
pub const MAX_CUSTOM_TITLE_CHARS: usize = 16;

impl AppState {
    /// Make an active, non-owner member an administrator with exactly `rights` (plus the
    /// baseline) and `custom_title`. Returns `false` when there is no such member.
    pub async fn appoint_chat_admin(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        rights: &[&str],
        custom_title: &str,
    ) -> Result<bool, sqlx::Error> {
        let appointed = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let changed = sqlx::query(
                "UPDATE chat_members SET \
                   role_id = (SELECT id FROM chat_roles WHERE room_id = $1 AND name = 'admin'), \
                   custom_title = $2 \
                 WHERE room_id = $3 AND user_id = $4 AND status = 'active' \
                   AND role_id <> (SELECT id FROM chat_roles WHERE room_id = $5 AND name = 'owner')",
            )
            .bind(room_id)
            .bind(custom_title)
            .bind(room_id)
            .bind(user_id)
            .bind(room_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if changed == 0 {
                transaction.rollback().await?;
                return Ok::<_, sqlx::Error>(false);
            }
            sqlx::query("DELETE FROM chat_admin_rights WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            for key in ADMIN_BASELINE
                .iter()
                .chain(rights.iter().filter(|key| !ADMIN_BASELINE.contains(key)))
            {
                sqlx::query(
                    "INSERT INTO chat_admin_rights (room_id, user_id, permission_key) \
                     VALUES ($1, $2, $3)",
                )
                .bind(room_id)
                .bind(user_id)
                .bind(*key)
                .execute(&mut *transaction)
                .await?;
            }
            // An administrator is not restricted: appointment lifts every restriction, as in
            // Telegram. Restricting them again requires dismissing them first.
            sqlx::query("DELETE FROM chat_member_restrictions WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(true)
        })?;
        Ok(appointed)
    }

    /// Move an administrator back to the `member` role. Returns `false` when the account is
    /// not an administrator of this chat.
    pub async fn dismiss_chat_admin(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chat_members SET \
                   role_id = (SELECT id FROM chat_roles WHERE room_id = $1 AND name = 'member') \
                 WHERE room_id = $2 AND user_id = $3 AND status = 'active' \
                   AND role_id = (SELECT id FROM chat_roles WHERE room_id = $4 AND name = 'admin')",
            )
            .bind(room_id)
            .bind(room_id)
            .bind(user_id)
            .bind(room_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            return Ok(false);
        }
        self.reset_admin_appointment(room_id, user_id).await?;
        Ok(true)
    }

    /// Forget an explicit right selection and the title; the member's role decides again.
    pub(crate) async fn reset_admin_appointment(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query("DELETE FROM chat_admin_rights WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "UPDATE chat_members SET custom_title = '' WHERE room_id = $1 AND user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await
        })
    }

    /// The rights an administrator holds, explicit or from the shared role. Empty for
    /// anyone who is not an active administrator.
    pub async fn chat_admin_rights(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<String>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT own.permission_key FROM chat_admin_rights AS own \
                 JOIN chat_members AS members ON members.room_id = own.room_id \
                   AND members.user_id = own.user_id AND members.status = 'active' \
                 JOIN chat_roles AS roles ON roles.id = members.role_id AND roles.name = 'admin' \
                 WHERE own.room_id = $1 AND own.user_id = $2 \
                 UNION \
                 SELECT grants.permission_key FROM chat_members AS members \
                 JOIN chat_roles AS roles ON roles.id = members.role_id AND roles.name = 'admin' \
                 JOIN chat_role_permissions AS grants ON grants.role_id = roles.id \
                 WHERE members.room_id = $3 AND members.user_id = $4 \
                   AND members.status = 'active' \
                   AND NOT EXISTS (SELECT 1 FROM chat_admin_rights AS custom \
                     WHERE custom.room_id = members.room_id AND custom.user_id = members.user_id) \
                 ORDER BY 1",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(room_id)
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }
}
