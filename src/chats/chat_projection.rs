//! Keeping the chat descriptor's projections honest after a membership write.
//!
//! `chats.member_count` is maintained by the database itself (the `chat_members_count_*`
//! triggers of migration `20261101000002`), inside the transaction of whichever statement
//! changed a membership. Two things are left for the application:
//!
//! * the member-limit trigger of the one-way supergroup upgrade, run inside the same
//!   membership transaction through [`upgrade_outgrown_group`];
//! * the in-memory chat cache (`AppState::chats`), which holds a copy of the descriptor and
//!   is refreshed from the row after the transaction commits.

use uuid::Uuid;

use crate::models::ChatMessage;
use crate::state::{with_pool, AppState};

/// Promote a `group` whose active membership has outgrown `ChatType::Group.member_limit()` to
/// a `supergroup`. One statement, so it joins the caller's transaction and sees the count the
/// triggers just wrote. Evaluates to whether the chat was upgraded.
///
/// A macro rather than a generic function because `rows_affected` is an inherent method of
/// each adapter's query result; inside `with_pool!` both arms are concrete.
macro_rules! upgrade_outgrown_group {
    ($executor:expr, $room_id:expr) => {
        sqlx::query(
            "UPDATE chats SET chat_type = 'supergroup' \
             WHERE id = $1 AND chat_type = 'group' AND member_count > $2 \
               AND deleted_at IS NULL",
        )
        .bind($room_id)
        .bind(
            $crate::chats::ChatType::Group
                .member_limit()
                .unwrap_or(i64::MAX),
        )
        .execute($executor)
        .await
        .map(|result| result.rows_affected() > 0)
    };
}
pub(crate) use upgrade_outgrown_group;

impl AppState {
    /// Re-read `chat_type` and `member_count` into the cached descriptor, and announce an
    /// upgrade to the chat's connections when `upgraded` is set.
    pub(crate) async fn sync_chat_projection(
        &self,
        room_id: Uuid,
        upgraded: bool,
    ) -> Result<(), sqlx::Error> {
        let row: Option<(String, i64)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT chat_type, CAST(member_count AS BIGINT) FROM chats \
                 WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some((chat_type, member_count)) = row else {
            return Ok(());
        };
        let Some(mut chat) = self.chat(room_id).await else {
            return Ok(());
        };
        if let Ok(chat_type) = chat_type.parse() {
            chat.chat_type = chat_type;
        }
        chat.member_count = member_count;
        self.cache_updated_chat(chat.clone()).await;
        if upgraded {
            tracing::info!(%room_id, "group upgraded to supergroup");
            chat.membership_status = None;
            chat.membership_role = None;
            self.broadcast(room_id, ChatMessage::ChatUpdated { chat })
                .await;
        }
        Ok(())
    }

    /// Run the member-limit upgrade on its own transaction-free statement. For membership
    /// writes that are a single statement already; the upgrade is idempotent and one-way, so
    /// running it right after the write is equivalent to running it inside.
    pub(crate) async fn settle_membership_change(&self, room_id: Uuid) -> Result<(), sqlx::Error> {
        let upgraded = with_pool!(self, |pool| { upgrade_outgrown_group!(pool, room_id) })?;
        self.sync_chat_projection(room_id, upgraded).await
    }
}
