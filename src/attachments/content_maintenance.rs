//! Reference bookkeeping for content-addressed attachments: which stored objects are no
//! longer referenced (orphan marking), and the one-time content-hash backfill for objects
//! stored before deduplication. Split from `content.rs`, which owns writing new attachments.

use anyhow::{Context, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::state::{with_pool, AppState};

impl AppState {
    pub(crate) async fn recompute_attachment_orphan_status(
        &self,
        attachment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            async {
                let group_key: Option<String> = sqlx::query_scalar(
                    "SELECT COALESCE(storage_key, CAST(id AS TEXT)) FROM attachments WHERE id = $1",
                )
                .bind(attachment_id)
                .fetch_optional(pool)
                .await?;
                let Some(group_key) = group_key else {
                    return Ok(());
                };
                let referenced: bool = sqlx::query_scalar(
                    "SELECT EXISTS (\
                       SELECT 1 FROM attachments a JOIN messages m ON m.attachment_id = a.id \
                       WHERE COALESCE(a.storage_key, CAST(a.id AS TEXT)) = $1 \
                       AND m.recalled_at IS NULL \
                       UNION ALL SELECT 1 FROM stickers s WHERE s.storage_key = $1 AND s.removed_at IS NULL UNION ALL SELECT 1 FROM user_saved_gifs g WHERE g.storage_key = $1 UNION ALL \
                       SELECT 1 FROM attachments a JOIN favorites f ON f.attachment_id = a.id \
                       WHERE COALESCE(a.storage_key, CAST(a.id AS TEXT)) = $1\
                     )",
                )
                .bind(&group_key)
                .fetch_one(pool)
                .await?;
                let query = if referenced {
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE COALESCE(storage_key, CAST(id AS TEXT)) = $1"
                } else {
                    "UPDATE attachments SET orphaned_at = $2 \
                     WHERE COALESCE(storage_key, CAST(id AS TEXT)) = $1 AND orphaned_at IS NULL"
                };
                let mut update = sqlx::query(query).bind(&group_key);
                if !referenced {
                    update = update.bind(Utc::now());
                }
                update.execute(pool).await?;
                Ok(())
            }
            .await
        })
    }

    /// Idempotently hash legacy UUID-keyed objects without moving them.
    pub(crate) async fn backfill_attachment_content_hashes(&self) -> Result<()> {
        let mut after: Option<Uuid> = None;
        loop {
            let ids: Vec<Uuid> = with_pool!(self, |pool| {
                match after {
                    Some(after) => {
                        sqlx::query_scalar(
                            "SELECT id FROM attachments WHERE content_hash IS NULL AND id > $1 \
                             ORDER BY id LIMIT 200",
                        )
                        .bind(after)
                        .fetch_all(pool)
                        .await
                    }
                    None => {
                        sqlx::query_scalar(
                            "SELECT id FROM attachments WHERE content_hash IS NULL ORDER BY id LIMIT 200",
                        )
                        .fetch_all(pool)
                        .await
                    }
                }
            })
            .context("load attachments pending content-hash backfill")?;
            let Some(&last) = ids.last() else {
                return Ok(());
            };
            after = Some(last);
            for id in ids {
                let key = id.simple().to_string();
                let hash = match self.attachment_store().hash_stored(&key).await {
                    Ok(hash) => hash,
                    Err(error) => {
                        tracing::warn!(
                            "backfill content hash for attachment {id} failed: {error:#}"
                        );
                        continue;
                    }
                };
                with_pool!(self, |pool| {
                    sqlx::query("UPDATE attachments SET content_hash = $1 WHERE id = $2")
                        .bind(&hash)
                        .bind(id)
                        .execute(pool)
                        .await
                        .map(|_| ())
                })
                .with_context(|| format!("store backfilled content hash for attachment {id}"))?;
            }
        }
    }
}
