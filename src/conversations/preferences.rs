use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::conversations::models::{
    ConversationPreferences, NotificationLevel, UpdateConversationPreferencesRequest,
};
use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
struct PreferenceRow {
    room_id: Uuid,
    is_pinned: bool,
    is_archived: bool,
    notification_level: String,
    muted_until: Option<DateTime<Utc>>,
    preferences_updated_at: DateTime<Utc>,
}

impl PreferenceRow {
    fn into_preferences(self) -> ConversationPreferences {
        ConversationPreferences {
            room_id: self.room_id,
            is_pinned: self.is_pinned,
            is_archived: self.is_archived,
            notification_level: NotificationLevel::from_database(&self.notification_level),
            muted_until: self.muted_until,
            updated_at: self.preferences_updated_at,
        }
    }
}

impl AppState {
    pub async fn get_preferences(
        &self,
        user_id: Uuid,
        room_id: Uuid,
    ) -> Result<Option<ConversationPreferences>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as::<_, PreferenceRow>(
                "SELECT room_id, is_pinned, is_archived, notification_level, muted_until, \
                 preferences_updated_at FROM room_memberships \
                 WHERE user_id = $1 AND room_id = $2 AND status = 'active'",
            )
            .bind(user_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await
            .map(|row| row.map(PreferenceRow::into_preferences))
        })
    }

    pub async fn update_preferences(
        &self,
        user_id: Uuid,
        room_id: Uuid,
        update: &UpdateConversationPreferencesRequest,
    ) -> Result<Option<ConversationPreferences>, sqlx::Error> {
        if update.is_empty() {
            return self.get_preferences(user_id, room_id).await;
        }
        let update_muted_until = update.muted_until.is_some();
        let muted_until = update.muted_until.flatten();
        let notification_level = update.notification_level.map(NotificationLevel::as_str);
        let updated = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE room_memberships SET \
                 is_pinned = COALESCE($1, is_pinned), \
                 is_archived = COALESCE($2, is_archived), \
                 notification_level = COALESCE($3, notification_level), \
                 muted_until = CASE WHEN $4 THEN $5 ELSE muted_until END, \
                 preferences_updated_at = CURRENT_TIMESTAMP \
                 WHERE user_id = $6 AND room_id = $7 AND status = 'active'",
            )
            .bind(update.is_pinned)
            .bind(update.is_archived)
            .bind(notification_level)
            .bind(update_muted_until)
            .bind(muted_until)
            .bind(user_id)
            .bind(room_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !updated {
            return Ok(None);
        }
        self.get_preferences(user_id, room_id).await
    }
}
