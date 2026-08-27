use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::models::{RoomTask, RoomTaskSource};
use crate::state::{with_pool, AppState};

#[derive(FromRow)]
struct TaskRow {
    id: Uuid,
    room_id: Uuid,
    title: String,
    status: String,
    assignee_id: Option<Uuid>,
    assignee_name: String,
    assignee_active: bool,
    created_by_id: Option<Uuid>,
    created_by_name: String,
    source_message_id: Option<Uuid>,
    source_sender: Option<String>,
    source_content: Option<String>,
    source_recalled_at: Option<DateTime<Utc>>,
    source_created_at: Option<DateTime<Utc>>,
    due_at: Option<DateTime<Utc>>,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TaskRow {
    fn into_task(self, viewer_id: Uuid, viewer_role: &str) -> RoomTask {
        let manages = matches!(viewer_role, "owner" | "admin");
        let can_update =
            manages || self.created_by_id == Some(viewer_id) || self.assignee_id == Some(viewer_id);
        let source = self.source_message_id.and_then(|message_id| {
            Some(RoomTaskSource {
                message_id,
                sender: self.source_sender?,
                excerpt: if self.source_recalled_at.is_none() {
                    truncate(self.source_content.as_deref().unwrap_or(""), 280)
                } else {
                    String::new()
                },
                recalled: self.source_recalled_at.is_some(),
                sent_at: self.source_created_at?,
            })
        });
        RoomTask {
            id: self.id,
            room_id: self.room_id,
            title: self.title,
            status: self.status,
            assignee_id: self.assignee_id,
            assignee_name: self.assignee_name,
            assignee_active: self.assignee_active,
            created_by_id: self.created_by_id,
            created_by_name: self.created_by_name,
            source,
            due_at: self.due_at,
            version: self.version,
            can_update,
            can_delete: manages,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl AppState {
    pub(super) async fn active_task_role(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT roles.name FROM room_memberships memberships \
                 JOIN room_roles roles ON roles.id = memberships.role_id \
                 JOIN rooms ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
                 WHERE memberships.room_id = $1 AND memberships.user_id = $2 \
                   AND memberships.status = 'active'",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn active_task_member(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        Ok(self.active_task_role(room_id, user_id).await?.is_some())
    }

    pub async fn room_tasks(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
    ) -> Result<Option<Vec<RoomTask>>, sqlx::Error> {
        let Some(role) = self.active_task_role(room_id, viewer_id).await? else {
            return Ok(None);
        };
        let rows = self.load_room_tasks(room_id, None).await?;
        Ok(Some(
            rows.into_iter()
                .map(|row| row.into_task(viewer_id, &role))
                .collect(),
        ))
    }

    pub(super) async fn room_task(
        &self,
        room_id: Uuid,
        task_id: Uuid,
        viewer_id: Uuid,
    ) -> Result<Option<RoomTask>, sqlx::Error> {
        let Some(role) = self.active_task_role(room_id, viewer_id).await? else {
            return Ok(None);
        };
        Ok(self
            .load_room_tasks(room_id, Some(task_id))
            .await?
            .into_iter()
            .next()
            .map(|row| row.into_task(viewer_id, &role)))
    }

    async fn load_room_tasks(
        &self,
        room_id: Uuid,
        task_id: Option<Uuid>,
    ) -> Result<Vec<TaskRow>, sqlx::Error> {
        let task_filter = task_id.map_or("", |_| "AND tasks.id = $2");
        let query = format!(
            "SELECT tasks.id, tasks.room_id, tasks.title, tasks.status, tasks.assignee_id, \
             COALESCE(NULLIF(assignees.display_name, ''), assignees.username, '') AS assignee_name, \
             CASE WHEN active_assignees.user_id IS NULL THEN FALSE ELSE TRUE END AS assignee_active, \
             tasks.created_by_id, \
             COALESCE(NULLIF(creators.display_name, ''), creators.username, tasks.created_by_name) \
               AS created_by_name, messages.id AS source_message_id, \
             messages.sender AS source_sender, messages.content AS source_content, \
             messages.recalled_at AS source_recalled_at, messages.created_at AS source_created_at, \
             tasks.due_at, tasks.version, tasks.created_at, tasks.updated_at \
             FROM room_tasks tasks \
             LEFT JOIN users assignees ON assignees.id = tasks.assignee_id \
             LEFT JOIN room_memberships active_assignees ON active_assignees.room_id = tasks.room_id \
               AND active_assignees.user_id = tasks.assignee_id AND active_assignees.status = 'active' \
             LEFT JOIN users creators ON creators.id = tasks.created_by_id \
             LEFT JOIN messages ON messages.id = tasks.source_message_id \
             WHERE tasks.room_id = $1 {task_filter} \
             ORDER BY CASE tasks.status WHEN 'open' THEN 0 WHEN 'in_progress' THEN 1 \
               WHEN 'done' THEN 2 ELSE 3 END, tasks.updated_at DESC, tasks.id DESC LIMIT 500"
        );
        with_pool!(self, |pool| {
            match task_id {
                Some(task_id) => {
                    sqlx::query_as(&query)
                        .bind(room_id)
                        .bind(task_id)
                        .fetch_all(pool)
                        .await
                }
                None => sqlx::query_as(&query).bind(room_id).fetch_all(pool).await,
            }
        })
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut value = value.chars().take(limit - 1).collect::<String>();
    value.push('…');
    value
}
