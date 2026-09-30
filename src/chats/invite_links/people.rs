//! Per-link statistics: who a link admitted, who it queued, and counting approved requests.

use uuid::Uuid;

use super::InviteLinkMember;
use crate::state::{with_pool, AppState};

impl AppState {
    /// The accounts a link admitted (`status = "active"`) or queued (`"pending"`), newest
    /// first, at most 200.
    pub async fn invite_link_members(
        &self,
        room_id: Uuid,
        link_id: Uuid,
        status: &str,
    ) -> Result<Vec<InviteLinkMember>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.display_name, \
                 users.avatar_emoji, chat_members.status, chat_members.requested_at, \
                 chat_members.joined_at \
                 FROM chat_members JOIN users ON users.id = chat_members.user_id \
                 WHERE chat_members.room_id = $1 AND chat_members.invite_link_id = $2 \
                   AND chat_members.status = $3 \
                 ORDER BY COALESCE(chat_members.joined_at, chat_members.requested_at) DESC, \
                   users.id LIMIT 200",
            )
            .bind(room_id)
            .bind(link_id)
            .bind(status)
            .fetch_all(pool)
            .await
        })
    }

    /// Count an approved join request against the link it arrived through. Called by the
    /// existing approve action (`activate_chat_member`) after the row turned active.
    pub async fn count_approved_invite_link_join(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chat_invite_links SET usage_count = usage_count + 1 \
                 WHERE id = (SELECT invite_link_id FROM chat_members \
                   WHERE room_id = $1 AND user_id = $2 AND status = 'active')",
            )
            .bind(room_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }
}
