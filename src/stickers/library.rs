//! One account's installed sticker sets: install, uninstall, archive and reorder.
//!
//! Every write runs in a transaction whose first statement upserts the account's
//! `user_sticker_state` row. On PostgreSQL that takes the row lock; on SQLite it takes the
//! database write lock. Either way two writes for one account cannot interleave between
//! reading the current order and writing the new one, so positions stay a permutation.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::catalogue::{SetRow, StickerScope, SET_COLUMNS};
use super::errors::StickerError;
use super::models::InstalledStickerSets;
use crate::state::{with_pool, AppState};

const LOCK_LIBRARY: &str = "INSERT INTO user_sticker_state (user_id, revision) VALUES ($1, 1) \
     ON CONFLICT (user_id) DO UPDATE SET revision = user_sticker_state.revision + 1";
const TOP_POSITION: &str =
    "SELECT COALESCE(MIN(position), 0) - 1 FROM user_sticker_sets WHERE user_id = $1";

/// One library mutation, applied under the account lock.
#[derive(Debug, Clone)]
enum LibraryWrite {
    Install(Uuid),
    Uninstall(Uuid),
    Archive(Uuid, bool),
    Reorder(Vec<Uuid>),
}

impl AppState {
    pub async fn installed_sticker_sets(
        &self,
        user_id: Uuid,
    ) -> Result<InstalledStickerSets, sqlx::Error> {
        let revision: Option<i64> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT revision FROM user_sticker_state WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(pool)
                .await
        })?;
        let query = format!(
            "{SET_COLUMNS}JOIN user_sticker_sets lib ON lib.set_id = sticker_sets.id \
             WHERE lib.user_id = $1 ORDER BY lib.position, lib.installed_at"
        );
        let rows: Vec<SetRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query).bind(user_id).fetch_all(pool).await
        })?;
        let archived: Vec<(Uuid, Option<DateTime<Utc>>)> = with_pool!(self, |pool| {
            sqlx::query_as("SELECT set_id, archived_at FROM user_sticker_sets WHERE user_id = $1")
                .bind(user_id)
                .fetch_all(pool)
                .await
        })?;
        let archived: HashMap<Uuid, bool> = archived
            .into_iter()
            .map(|(set_id, archived_at)| (set_id, archived_at.is_some()))
            .collect();
        let mut stickers: HashMap<Uuid, Vec<_>> = HashMap::new();
        for sticker in self
            .load_stickers(&StickerScope::InstalledBy(user_id))
            .await?
        {
            stickers.entry(sticker.set_id).or_default().push(sticker);
        }
        let sets = rows
            .into_iter()
            .map(|row| {
                let state = archived.get(&row.id).copied().unwrap_or(false);
                let list = stickers.remove(&row.id).unwrap_or_default();
                row.into_set(list, Some(state))
            })
            .collect();
        Ok(InstalledStickerSets {
            revision: revision.unwrap_or(0),
            sets,
        })
    }

    /// Install at the top, or bring an archived set back to the top. Idempotent.
    pub async fn install_sticker_set(
        &self,
        user_id: Uuid,
        set_id: Uuid,
    ) -> Result<InstalledStickerSets, StickerError> {
        self.write_library(user_id, LibraryWrite::Install(set_id))
            .await
    }

    /// Remove from the library. Idempotent.
    pub async fn uninstall_sticker_set(
        &self,
        user_id: Uuid,
        set_id: Uuid,
    ) -> Result<InstalledStickerSets, StickerError> {
        self.write_library(user_id, LibraryWrite::Uninstall(set_id))
            .await
    }

    /// Archive, or un-archive (which also moves the set to the top, as Telegram does).
    pub async fn archive_sticker_set(
        &self,
        user_id: Uuid,
        set_id: Uuid,
        archived: bool,
    ) -> Result<InstalledStickerSets, StickerError> {
        self.write_library(user_id, LibraryWrite::Archive(set_id, archived))
            .await
    }

    /// See [`super::models::ReorderStickerSetsRequest`] for the merge rule.
    pub async fn reorder_sticker_sets(
        &self,
        user_id: Uuid,
        set_ids: Vec<Uuid>,
    ) -> Result<InstalledStickerSets, StickerError> {
        self.write_library(user_id, LibraryWrite::Reorder(set_ids))
            .await
    }

    async fn write_library(
        &self,
        user_id: Uuid,
        write: LibraryWrite,
    ) -> Result<InstalledStickerSets, StickerError> {
        let now = Utc::now();
        let result: Result<(), StickerError> = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query(LOCK_LIBRARY)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                match &write {
                    LibraryWrite::Install(set_id) => {
                        let installed: Option<Option<DateTime<Utc>>> = sqlx::query_scalar(
                            "SELECT archived_at FROM user_sticker_sets \
                             WHERE user_id = $1 AND set_id = $2",
                        )
                        .bind(user_id)
                        .bind(set_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                        let top: i64 = sqlx::query_scalar(TOP_POSITION)
                            .bind(user_id)
                            .fetch_one(&mut *tx)
                            .await?;
                        match installed {
                            None => {
                                let exists: bool = sqlx::query_scalar(
                                    "SELECT EXISTS(SELECT 1 FROM sticker_sets WHERE id = $1)",
                                )
                                .bind(set_id)
                                .fetch_one(&mut *tx)
                                .await?;
                                if !exists {
                                    return Err(StickerError::NotFound("sticker_set_not_found"));
                                }
                                sqlx::query(
                                    "INSERT INTO user_sticker_sets \
                                     (user_id, set_id, position, installed_at) \
                                     VALUES ($1, $2, $3, $4)",
                                )
                                .bind(user_id)
                                .bind(set_id)
                                .bind(top)
                                .bind(now)
                                .execute(&mut *tx)
                                .await?;
                            }
                            Some(Some(_)) => {
                                sqlx::query(
                                    "UPDATE user_sticker_sets SET archived_at = NULL, \
                                     position = $1 WHERE user_id = $2 AND set_id = $3",
                                )
                                .bind(top)
                                .bind(user_id)
                                .bind(set_id)
                                .execute(&mut *tx)
                                .await?;
                            }
                            Some(None) => {}
                        }
                    }
                    LibraryWrite::Uninstall(set_id) => {
                        sqlx::query(
                            "DELETE FROM user_sticker_sets WHERE user_id = $1 AND set_id = $2",
                        )
                        .bind(user_id)
                        .bind(set_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                    LibraryWrite::Archive(set_id, archived) => {
                        let top: i64 = sqlx::query_scalar(TOP_POSITION)
                            .bind(user_id)
                            .fetch_one(&mut *tx)
                            .await?;
                        let changed = if *archived {
                            sqlx::query(
                                "UPDATE user_sticker_sets \
                                 SET archived_at = COALESCE(archived_at, $1) \
                                 WHERE user_id = $2 AND set_id = $3",
                            )
                            .bind(now)
                            .bind(user_id)
                            .bind(set_id)
                            .execute(&mut *tx)
                            .await?
                        } else {
                            sqlx::query(
                                "UPDATE user_sticker_sets SET archived_at = NULL, position = $1 \
                                 WHERE user_id = $2 AND set_id = $3 AND archived_at IS NOT NULL",
                            )
                            .bind(top)
                            .bind(user_id)
                            .bind(set_id)
                            .execute(&mut *tx)
                            .await?
                        }
                        .rows_affected();
                        if changed == 0 {
                            let installed: bool = sqlx::query_scalar(
                                "SELECT EXISTS(SELECT 1 FROM user_sticker_sets \
                                 WHERE user_id = $1 AND set_id = $2)",
                            )
                            .bind(user_id)
                            .bind(set_id)
                            .fetch_one(&mut *tx)
                            .await?;
                            if !installed {
                                return Err(StickerError::NotFound("sticker_set_not_installed"));
                            }
                        }
                    }
                    LibraryWrite::Reorder(requested) => {
                        let current: Vec<Uuid> = sqlx::query_scalar(
                            "SELECT set_id FROM user_sticker_sets WHERE user_id = $1 \
                             ORDER BY position, installed_at",
                        )
                        .bind(user_id)
                        .fetch_all(&mut *tx)
                        .await?;
                        for (position, set_id) in
                            merged_order(requested, &current).into_iter().enumerate()
                        {
                            sqlx::query(
                                "UPDATE user_sticker_sets SET position = $1 \
                                 WHERE user_id = $2 AND set_id = $3",
                            )
                            .bind(position as i64)
                            .bind(user_id)
                            .bind(set_id)
                            .execute(&mut *tx)
                            .await?;
                        }
                    }
                }
                tx.commit().await?;
                Ok(())
            }
            .await
        });
        result?;
        Ok(self.installed_sticker_sets(user_id).await?)
    }
}

/// Requested ids that are installed, in the requested order, then every other installed
/// set in its current order. Duplicates and unknown ids in the request are dropped.
fn merged_order(requested: &[Uuid], current: &[Uuid]) -> Vec<Uuid> {
    let mut order: Vec<Uuid> = Vec::with_capacity(current.len());
    for id in requested {
        if current.contains(id) && !order.contains(id) {
            order.push(*id);
        }
    }
    for id in current {
        if !order.contains(id) {
            order.push(*id);
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::merged_order;
    use uuid::Uuid;

    #[test]
    fn a_stale_reorder_keeps_sets_it_did_not_mention() {
        let [a, b, c, unknown] = [(); 4].map(|_| Uuid::new_v4());
        assert_eq!(merged_order(&[c, a, unknown, c], &[a, b, c]), vec![c, a, b]);
    }
}
