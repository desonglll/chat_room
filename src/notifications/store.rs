use chrono::Utc;
use uuid::Uuid;

use super::models::{
    NotificationAccountState, NotificationEvent, NotificationKind, NotificationPage,
    NotificationQuery, NotificationView,
};
use super::projection::{NotificationRow, NOTIFICATION_SELECT};
use crate::state::{with_pool, AppState};

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
        let sql = format!(
            "{NOTIFICATION_SELECT} WHERE notifications.recipient_id = $1 \
             AND ($2 IS NULL OR notifications.kind = $2) \
             AND ($3 IS NULL OR notifications.created_at < $3 OR \
               (notifications.created_at = $3 AND notifications.id < $4)) \
             ORDER BY notifications.created_at DESC, notifications.id DESC LIMIT $5"
        );
        let mut rows: Vec<NotificationRow> = with_pool!(self, |pool| {
            sqlx::query_as(&sql)
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

    pub(crate) async fn notification_for_push(
        &self,
        recipient_id: Uuid,
        notification_id: &str,
    ) -> Result<Option<NotificationView>, sqlx::Error> {
        let sql = format!(
            "{NOTIFICATION_SELECT} WHERE notifications.recipient_id = $1 \
             AND notifications.id = $2 LIMIT 1"
        );
        let row: Option<NotificationRow> = with_pool!(self, |pool| {
            sqlx::query_as(&sql)
                .bind(recipient_id)
                .bind(notification_id)
                .fetch_optional(pool)
                .await
        })?;
        row.map(NotificationRow::into_view).transpose()
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
