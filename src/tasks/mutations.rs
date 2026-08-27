use chrono::Utc;
use uuid::Uuid;

use super::models::{
    valid_status, valid_title, CreateRoomTaskRequest, RoomTask, TaskMutation, UpdateRoomTaskRequest,
};
use crate::state::{with_pool, AppState};

impl AppState {
    pub(super) async fn create_room_task(
        &self,
        room_id: Uuid,
        actor_id: Uuid,
        actor_name: &str,
        request: CreateRoomTaskRequest,
    ) -> Result<TaskMutation<RoomTask>, sqlx::Error> {
        let title = request.title.trim();
        if !valid_title(title) {
            return Ok(TaskMutation::InvalidValue);
        }
        let task_id = Uuid::new_v4();
        let now = Utc::now();
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO room_tasks \
                 (id, room_id, title, status, assignee_id, created_by_id, created_by_name, \
                  source_message_id, due_at, version, created_at, updated_at) \
                 SELECT $1, $2, $3, 'open', $4, $5, $6, $7, $8, 1, $9, $9 \
                 WHERE EXISTS (SELECT 1 FROM room_memberships actor \
                   WHERE actor.room_id = $2 AND actor.user_id = $5 AND actor.status = 'active') \
                 AND ($4 IS NULL OR EXISTS (SELECT 1 FROM room_memberships assignee \
                   WHERE assignee.room_id = $2 AND assignee.user_id = $4 AND assignee.status = 'active')) \
                 AND ($7 IS NULL OR EXISTS (SELECT 1 FROM messages source \
                   WHERE source.id = $7 AND source.room_id = $2 AND source.recalled_at IS NULL))",
            )
            .bind(task_id)
            .bind(room_id)
            .bind(title)
            .bind(request.assignee_id)
            .bind(actor_id)
            .bind(actor_name)
            .bind(request.source_message_id)
            .bind(request.due_at)
            .bind(now)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !inserted {
            if !self.active_task_member(room_id, actor_id).await? {
                return Ok(TaskMutation::Forbidden);
            }
            if let Some(assignee_id) = request.assignee_id {
                if !self.active_task_member(room_id, assignee_id).await? {
                    return Ok(TaskMutation::InvalidAssignee);
                }
            }
            return Ok(TaskMutation::InvalidSource);
        }
        Ok(self
            .room_task(room_id, task_id, actor_id)
            .await?
            .map(TaskMutation::Applied)
            .unwrap_or(TaskMutation::NotFound))
    }

    pub(super) async fn update_room_task(
        &self,
        room_id: Uuid,
        task_id: Uuid,
        actor_id: Uuid,
        request: UpdateRoomTaskRequest,
    ) -> Result<TaskMutation<RoomTask>, sqlx::Error> {
        let title = request.title.trim();
        if !valid_title(title) || !valid_status(&request.status) || request.version < 1 {
            return Ok(TaskMutation::InvalidValue);
        }
        let now = Utc::now();
        let updated = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE room_tasks SET title = $1, status = $2, assignee_id = $3, due_at = $4, \
                 version = version + 1, updated_at = $5 \
                 WHERE id = $6 AND room_id = $7 AND version = $8 \
                 AND ($3 IS NULL OR EXISTS (SELECT 1 FROM room_memberships target \
                   WHERE target.room_id = $7 AND target.user_id = $3 AND target.status = 'active')) \
                 AND EXISTS (SELECT 1 FROM room_memberships actor \
                   JOIN room_roles role ON role.id = actor.role_id \
                   WHERE actor.room_id = $7 AND actor.user_id = $9 AND actor.status = 'active' \
                   AND (room_tasks.created_by_id = $9 OR room_tasks.assignee_id = $9 \
                     OR role.name IN ('owner', 'admin')))",
            )
            .bind(title)
            .bind(&request.status)
            .bind(request.assignee_id)
            .bind(request.due_at)
            .bind(now)
            .bind(task_id)
            .bind(room_id)
            .bind(request.version)
            .bind(actor_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !updated {
            return self
                .classify_update_failure(
                    room_id,
                    task_id,
                    actor_id,
                    request.assignee_id,
                    request.version,
                )
                .await;
        }
        Ok(self
            .room_task(room_id, task_id, actor_id)
            .await?
            .map(TaskMutation::Applied)
            .unwrap_or(TaskMutation::NotFound))
    }

    async fn classify_update_failure(
        &self,
        room_id: Uuid,
        task_id: Uuid,
        actor_id: Uuid,
        assignee_id: Option<Uuid>,
        expected_version: i64,
    ) -> Result<TaskMutation<RoomTask>, sqlx::Error> {
        let Some(role) = self.active_task_role(room_id, actor_id).await? else {
            return Ok(TaskMutation::Forbidden);
        };
        let Some((created_by, current_assignee, version)) =
            self.task_metadata(room_id, task_id).await?
        else {
            return Ok(TaskMutation::NotFound);
        };
        if !matches!(role.as_str(), "owner" | "admin")
            && created_by != Some(actor_id)
            && current_assignee != Some(actor_id)
        {
            return Ok(TaskMutation::Forbidden);
        }
        if let Some(assignee_id) = assignee_id {
            if !self.active_task_member(room_id, assignee_id).await? {
                return Ok(TaskMutation::InvalidAssignee);
            }
        }
        Ok(if version != expected_version {
            TaskMutation::Conflict
        } else {
            TaskMutation::NotFound
        })
    }

    pub(super) async fn delete_room_task(
        &self,
        room_id: Uuid,
        task_id: Uuid,
        actor_id: Uuid,
    ) -> Result<TaskMutation<()>, sqlx::Error> {
        let deleted = with_pool!(self, |pool| {
            sqlx::query(
                "DELETE FROM room_tasks WHERE id = $1 AND room_id = $2 \
                 AND EXISTS (SELECT 1 FROM room_memberships actor \
                   JOIN room_roles role ON role.id = actor.role_id \
                   WHERE actor.room_id = $2 AND actor.user_id = $3 AND actor.status = 'active' \
                     AND role.name IN ('owner', 'admin'))",
            )
            .bind(task_id)
            .bind(room_id)
            .bind(actor_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if deleted {
            return Ok(TaskMutation::Applied(()));
        }
        if self.active_task_role(room_id, actor_id).await?.is_none() {
            return Ok(TaskMutation::Forbidden);
        }
        Ok(if self.task_metadata(room_id, task_id).await?.is_none() {
            TaskMutation::NotFound
        } else {
            TaskMutation::Forbidden
        })
    }

    async fn task_metadata(
        &self,
        room_id: Uuid,
        task_id: Uuid,
    ) -> Result<Option<(Option<Uuid>, Option<Uuid>, i64)>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT created_by_id, assignee_id, version FROM room_tasks \
                 WHERE room_id = $1 AND id = $2",
            )
            .bind(room_id)
            .bind(task_id)
            .fetch_optional(pool)
            .await
        })
    }
}
