use chrono::Utc;
use tokio::io::AsyncReadExt;

use crate::ai::{VisionImage, VisionLimits, VisualProjection};
use crate::state::{with_pool, SharedState};

use super::models::AiCitationSource;
use super::run_store::AiRunExecution;

pub(super) async fn load_cached_projection(
    state: &SharedState,
    execution: &AiRunExecution,
    source: &AiCitationSource,
    model: &str,
    prompt_version: i64,
) -> anyhow::Result<Option<VisualProjection>> {
    let Some(room_id) = execution.room_id else {
        return Ok(None);
    };
    let Some(attachment) = source.attachment.as_ref() else {
        return Ok(None);
    };
    let projection: Option<String> = with_pool!(state, |pool| {
        sqlx::query_scalar(
            "SELECT projections.projection FROM attachment_visual_projections projections \
             JOIN attachments ON attachments.id = projections.attachment_id \
             JOIN messages ON messages.attachment_id = attachments.id \
             WHERE projections.attachment_id = $1 AND projections.room_id = $2 \
               AND projections.model = $3 AND projections.prompt_version = $4 \
               AND attachments.room_id = $2 AND messages.id = $5 AND messages.room_id = $2 \
               AND messages.recalled_at IS NULL AND attachments.is_sensitive = FALSE \
               AND EXISTS (SELECT 1 FROM room_memberships \
                 WHERE room_memberships.room_id = $2 AND room_memberships.user_id = $6 \
                   AND room_memberships.status = 'active') LIMIT 1",
        )
        .bind(attachment.id)
        .bind(room_id)
        .bind(model)
        .bind(prompt_version)
        .bind(source.message_id)
        .bind(execution.user_id)
        .fetch_optional(pool)
        .await
    })?;
    projection
        .map(|projection| serde_json::from_str(&projection).map_err(Into::into))
        .transpose()
}

pub(super) async fn store_visual_projection(
    state: &SharedState,
    execution: &AiRunExecution,
    source: &AiCitationSource,
    model: &str,
    prompt_version: i64,
    projection: &VisualProjection,
) -> anyhow::Result<bool> {
    let Some(room_id) = execution.room_id else {
        return Ok(false);
    };
    let Some(attachment) = source.attachment.as_ref() else {
        return Ok(false);
    };
    let encoded = serde_json::to_string(projection)?;
    let search_text = projection.search_text();
    let now = Utc::now();
    with_pool!(state, |pool| {
        let mut transaction = pool.begin().await?;
        let stored = sqlx::query(
            "INSERT INTO attachment_visual_projections \
             (attachment_id, room_id, model, prompt_version, projection, search_text, \
              created_at, updated_at) \
             SELECT attachments.id, attachments.room_id, $1, $2, $3, $4, $5, $5 \
             FROM attachments JOIN messages ON messages.attachment_id = attachments.id \
             WHERE attachments.id = $6 AND attachments.room_id = $7 \
               AND messages.id = $8 AND messages.room_id = $7 \
               AND messages.recalled_at IS NULL AND attachments.is_sensitive = FALSE \
               AND EXISTS (SELECT 1 FROM room_memberships \
                 WHERE room_memberships.room_id = $7 AND room_memberships.user_id = $9 \
                   AND room_memberships.status = 'active') \
             ON CONFLICT (attachment_id, model, prompt_version) DO UPDATE SET \
               projection = excluded.projection, search_text = excluded.search_text, \
               updated_at = excluded.updated_at",
        )
        .bind(model)
        .bind(prompt_version)
        .bind(&encoded)
        .bind(&search_text)
        .bind(now)
        .bind(attachment.id)
        .bind(room_id)
        .bind(source.message_id)
        .bind(execution.user_id)
        .execute(&mut *transaction)
        .await?;
        if stored.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO message_index_outbox \
             (message_id, operation, next_attempt_at, updated_at) \
             VALUES ($1, 'upsert', $2, $2) \
             ON CONFLICT (message_id) DO UPDATE SET operation = 'upsert', attempt_count = 0, \
               generation = message_index_outbox.generation + 1, \
               next_attempt_at = excluded.next_attempt_at, last_error = NULL, \
               updated_at = excluded.updated_at",
        )
        .bind(source.message_id)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    })
}

pub(super) async fn read_authorized_image(
    state: &SharedState,
    execution: &AiRunExecution,
    source: &AiCitationSource,
    limits: VisionLimits,
) -> anyhow::Result<Option<VisionImage>> {
    let Some(room_id) = execution.room_id else {
        return Ok(None);
    };
    let Some(attachment) = source.attachment.as_ref() else {
        return Ok(None);
    };
    let row: Option<(String, String, i64, Option<String>)> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT attachments.file_name, attachments.mime_type, attachments.size_bytes, \
             attachments.storage_key FROM attachments JOIN messages \
             ON messages.attachment_id = attachments.id \
             WHERE attachments.id = $1 AND messages.id = $2 AND messages.room_id = $3 \
               AND attachments.room_id = $3 \
               AND messages.recalled_at IS NULL AND attachments.is_sensitive = FALSE \
               AND EXISTS (SELECT 1 FROM room_memberships \
                 WHERE room_memberships.room_id = $3 AND room_memberships.user_id = $4 \
                   AND room_memberships.status = 'active') LIMIT 1",
        )
        .bind(attachment.id)
        .bind(source.message_id)
        .bind(room_id)
        .bind(execution.user_id)
        .fetch_optional(pool)
        .await
    })?;
    let Some((file_name, content_type, size_bytes, storage_key)) = row else {
        return Ok(None);
    };
    let size = u64::try_from(size_bytes)?;
    if size == 0 || size > limits.max_image_bytes || !content_type.starts_with("image/") {
        return Ok(None);
    }
    let storage_key = storage_key.unwrap_or_else(|| attachment.id.simple().to_string());
    let reader = state
        .attachment_store()
        .open_range(&storage_key, 0, size)
        .await?;
    let mut bytes = Vec::with_capacity(usize::try_from(size)?);
    reader
        .take(limits.max_image_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .await?;
    if bytes.is_empty() || bytes.len() as u64 > limits.max_image_bytes {
        return Ok(None);
    }
    Ok(Some(VisionImage {
        content_type,
        file_name,
        bytes,
    }))
}
