use std::str::FromStr;

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::models::{
    NotificationAccountState, NotificationActor, NotificationCursor, NotificationEvent,
    NotificationKind, NotificationPage, NotificationQuery, NotificationView,
};
use crate::state::{with_pool, AppState};

#[derive(FromRow)]
struct NotificationRow {
    id: String,
    kind: String,
    actor_id: Option<Uuid>,
    actor_username: Option<String>,
    actor_display_name: Option<String>,
    actor_avatar_emoji: Option<String>,
    room_id: Option<Uuid>,
    room_name: Option<String>,
    message_id: Option<Uuid>,
    message_content: Option<String>,
    run_id: Option<Uuid>,
    stored_summary: String,
    source_available: bool,
    created_at: DateTime<Utc>,
    read_at: Option<DateTime<Utc>>,
}

impl NotificationRow {
    fn cursor(&self) -> NotificationCursor {
        NotificationCursor {
            created_at: self.created_at,
            id: self.id.clone(),
        }
    }

    fn into_view(self) -> Result<NotificationView, sqlx::Error> {
        let kind = NotificationKind::from_str(&self.kind)
            .map_err(|_| sqlx::Error::Protocol("invalid notification kind".into()))?;
        let actor = if self.source_available || kind == NotificationKind::FriendRequest {
            self.actor_id.and_then(|id| {
                Some(NotificationActor {
                    id,
                    username: self.actor_username?,
                    display_name: self.actor_display_name?,
                    avatar_emoji: self.actor_avatar_emoji?,
                })
            })
        } else {
            None
        };
        let summary = summary(
            kind,
            actor.as_ref(),
            self.room_name.as_deref(),
            self.message_content.as_deref(),
            self.source_available,
            &self.stored_summary,
        );
        Ok(NotificationView {
            id: self.id,
            kind,
            actor,
            room_id: self.source_available.then_some(self.room_id).flatten(),
            room_name: self.source_available.then_some(self.room_name).flatten(),
            message_id: self.source_available.then_some(self.message_id).flatten(),
            run_id: self.source_available.then_some(self.run_id).flatten(),
            summary,
            source_available: self.source_available,
            created_at: self.created_at,
            read_at: self.read_at,
        })
    }
}

impl AppState {
    pub async fn record_notification(
        &self,
        event: &NotificationEvent,
    ) -> Result<bool, sqlx::Error> {
        if event.dedupe_key.is_empty() || event.dedupe_key.chars().count() > 300 {
            return Err(sqlx::Error::Protocol(
                "invalid notification dedupe key".into(),
            ));
        }
        let summary: String = event.summary.chars().take(160).collect();
        let id = Uuid::new_v4().to_string();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO notifications \
                 (id, recipient_id, kind, actor_id, room_id, message_id, run_id, summary, \
                  dedupe_key, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                 ON CONFLICT(dedupe_key) DO NOTHING",
            )
            .bind(id)
            .bind(event.recipient_id)
            .bind(event.kind.as_str())
            .bind(event.actor_id)
            .bind(event.room_id)
            .bind(event.message_id)
            .bind(event.run_id)
            .bind(summary)
            .bind(&event.dedupe_key)
            .bind(event.created_at)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    pub async fn list_notifications(
        &self,
        recipient_id: Uuid,
        query: &NotificationQuery,
    ) -> Result<NotificationPage, sqlx::Error> {
        let cursor_at = query.cursor.as_ref().map(|cursor| cursor.created_at);
        let cursor_id = query.cursor.as_ref().map(|cursor| cursor.id.as_str());
        let kind = query.kind.map(NotificationKind::as_str);
        let mut rows: Vec<NotificationRow> = with_pool!(self, |pool| {
            sqlx::query_as(NOTIFICATION_SELECT)
                .bind(recipient_id)
                .bind(kind)
                .bind(cursor_at)
                .bind(cursor_id)
                .bind(query.limit + 1)
                .fetch_all(pool)
                .await
        })?;
        let has_more = rows.len() > query.limit as usize;
        if has_more {
            rows.pop();
        }
        let next_cursor = has_more
            .then(|| rows.last().map(NotificationRow::cursor))
            .flatten()
            .map(|cursor| cursor.to_string());
        let items = rows
            .into_iter()
            .map(NotificationRow::into_view)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(NotificationPage { items, next_cursor })
    }

    pub async fn mark_notification_read(
        &self,
        recipient_id: Uuid,
        id: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE notifications SET read_at = COALESCE(read_at, $1) \
                 WHERE recipient_id = $2 AND id = $3",
            )
            .bind(Utc::now())
            .bind(recipient_id)
            .bind(id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    pub async fn mark_all_notifications_read(
        &self,
        recipient_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE notifications SET read_at = $1 \
                 WHERE recipient_id = $2 AND read_at IS NULL",
            )
            .bind(Utc::now())
            .bind(recipient_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })
    }

    pub async fn notification_unread_count(&self, recipient_id: Uuid) -> Result<i64, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM notifications \
                 WHERE recipient_id = $1 AND read_at IS NULL",
            )
            .bind(recipient_id)
            .fetch_one(pool)
            .await
        })
    }

    pub(crate) async fn notification_account_state(
        &self,
        recipient_id: Uuid,
    ) -> Result<NotificationAccountState, sqlx::Error> {
        let (unread_count, latest_notification_id): (i64, Option<String>) =
            with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT COUNT(*) FILTER (WHERE read_at IS NULL) AS unread_count, \
                     (SELECT latest.id FROM notifications AS latest \
                      WHERE latest.recipient_id = $1 \
                      ORDER BY latest.created_at DESC, latest.id DESC LIMIT 1) \
                      AS latest_notification_id \
                     FROM notifications WHERE recipient_id = $1",
                )
                .bind(recipient_id)
                .fetch_one(pool)
                .await
            })?;
        Ok(NotificationAccountState {
            unread_count,
            latest_notification_id,
        })
    }
}

const NOTIFICATION_SELECT: &str = "SELECT notifications.id, notifications.kind, \
    CASE WHEN notifications.actor_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM user_blocks \
      WHERE (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) \
         OR (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
      THEN actor.id ELSE NULL END AS actor_id, \
    CASE WHEN notifications.actor_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM user_blocks \
      WHERE (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) \
         OR (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
      THEN actor.username ELSE NULL END AS actor_username, \
    CASE WHEN notifications.actor_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM user_blocks \
      WHERE (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) \
         OR (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
      THEN actor.display_name ELSE NULL END AS actor_display_name, \
    CASE WHEN notifications.actor_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM user_blocks \
      WHERE (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) \
         OR (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
      THEN actor.avatar_emoji ELSE NULL END AS actor_avatar_emoji, \
    notifications.room_id, rooms.name AS room_name, notifications.message_id, \
    messages.content AS message_content, notifications.run_id, \
    notifications.summary AS stored_summary, \
    CASE WHEN notifications.kind IN ('mention', 'reply') THEN \
      messages.id IS NOT NULL AND messages.recalled_at IS NULL AND rooms.deleted_at IS NULL \
      AND viewer.status = 'active' \
      AND messages.created_at >= COALESCE(viewer.joined_at, viewer.requested_at) \
      AND NOT EXISTS (SELECT 1 FROM user_blocks WHERE \
        (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) OR \
        (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
    WHEN notifications.kind = 'room_join_request' THEN \
      rooms.deleted_at IS NULL AND viewer.status = 'active' AND EXISTS ( \
        SELECT 1 FROM room_role_permissions WHERE role_id = viewer.role_id \
          AND permission_key = 'members.review') \
      AND NOT EXISTS (SELECT 1 FROM user_blocks WHERE \
        (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) OR \
        (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
    WHEN notifications.kind = 'ai_run_completed' THEN \
      ai_runs.id IS NOT NULL AND (notifications.room_id IS NULL OR \
        (rooms.deleted_at IS NULL AND viewer.status = 'active')) \
    WHEN notifications.kind = 'friend_request' THEN notifications.actor_id IS NOT NULL \
      AND NOT EXISTS (SELECT 1 FROM user_blocks WHERE \
        (blocker_id = notifications.recipient_id AND blocked_id = notifications.actor_id) OR \
        (blocker_id = notifications.actor_id AND blocked_id = notifications.recipient_id)) \
    ELSE FALSE END AS source_available, notifications.created_at, notifications.read_at \
    FROM notifications LEFT JOIN users AS actor ON actor.id = notifications.actor_id \
    LEFT JOIN rooms ON rooms.id = notifications.room_id \
    LEFT JOIN messages ON messages.id = notifications.message_id \
    LEFT JOIN ai_runs ON ai_runs.id = notifications.run_id \
    LEFT JOIN room_memberships AS viewer ON viewer.room_id = notifications.room_id \
      AND viewer.user_id = notifications.recipient_id \
    WHERE notifications.recipient_id = $1 AND ($2 IS NULL OR notifications.kind = $2) \
      AND ($3 IS NULL OR notifications.created_at < $3 OR \
        (notifications.created_at = $3 AND notifications.id < $4)) \
    ORDER BY notifications.created_at DESC, notifications.id DESC LIMIT $5";

fn summary(
    kind: NotificationKind,
    actor: Option<&NotificationActor>,
    room_name: Option<&str>,
    message_content: Option<&str>,
    source_available: bool,
    stored_summary: &str,
) -> String {
    if source_available && !stored_summary.trim().is_empty() {
        return stored_summary.to_owned();
    }
    let actor_name = actor.map(|actor| {
        if actor.display_name.trim().is_empty() {
            actor.username.as_str()
        } else {
            actor.display_name.as_str()
        }
    });
    match (kind, source_available) {
        (NotificationKind::FriendRequest, true) => {
            format!(
                "{} sent you a friend request",
                actor_name.unwrap_or("Someone")
            )
        }
        (NotificationKind::FriendRequest, false) => "Friend request activity".into(),
        (NotificationKind::RoomJoinRequest, true) => format!(
            "{} requested to join {}",
            actor_name.unwrap_or("Someone"),
            room_name.unwrap_or("a room")
        ),
        (NotificationKind::Mention, true) => message_summary(message_content, "Mentioned you"),
        (NotificationKind::Reply, true) => message_summary(message_content, "Replied to you"),
        (NotificationKind::AiRunCompleted, true) => "AI run completed".into(),
        (NotificationKind::AiRunCompleted, false) => "AI run is no longer available".into(),
        _ => "Source is no longer available".into(),
    }
}

fn message_summary(content: Option<&str>, fallback: &str) -> String {
    let content = content.unwrap_or("").trim();
    if content.is_empty() {
        return fallback.into();
    }
    let mut excerpt: String = content.chars().take(120).collect();
    if content.chars().count() > 120 {
        excerpt.push_str("...");
    }
    excerpt
}
