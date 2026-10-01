//! TG-501 folder storage: rows in `chat_folders`, chat lists in `chat_folder_chats`.

use uuid::Uuid;

use super::{ChatFolder, ChatFolderWrite};
use crate::state::{with_pool, AppState};

type FolderRow = (Uuid, String, String, String, bool, bool, bool);

impl AppState {
    pub(super) async fn user_folders(&self, user_id: Uuid) -> Result<Vec<ChatFolder>, sqlx::Error> {
        let rows: Vec<FolderRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, title, emoji, include_types, exclude_muted, exclude_read, exclude_archived \
                 FROM chat_folders WHERE user_id = $1 ORDER BY position, created_at",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        let mut folders = Vec::with_capacity(rows.len());
        for (id, title, emoji, types, exclude_muted, exclude_read, exclude_archived) in rows {
            let chats: Vec<(Uuid, String)> = with_pool!(self, |pool| {
                sqlx::query_as("SELECT room_id, mode FROM chat_folder_chats WHERE folder_id = $1")
                    .bind(id)
                    .fetch_all(pool)
                    .await
            })?;
            let mut include_chat_ids = Vec::new();
            let mut exclude_chat_ids = Vec::new();
            for (room_id, mode) in chats {
                // Read-time authorization: a chat the owner has left is no longer listed.
                if !self.can_read_chat(room_id, user_id).await? {
                    continue;
                }
                if mode == "include" {
                    include_chat_ids.push(room_id);
                } else {
                    exclude_chat_ids.push(room_id);
                }
            }
            folders.push(ChatFolder {
                id,
                title,
                emoji,
                include_types: serde_json::from_str(&types).unwrap_or_default(),
                include_chat_ids,
                exclude_chat_ids,
                exclude_muted,
                exclude_read,
                exclude_archived,
            });
        }
        Ok(folders)
    }

    pub(super) async fn write_folder_chats(
        &self,
        folder_id: Uuid,
        write: &ChatFolderWrite,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query("DELETE FROM chat_folder_chats WHERE folder_id = $1")
                    .bind(folder_id)
                    .execute(&mut *tx)
                    .await?;
                for (ids, mode) in [(&write.include_chat_ids, "include"), (&write.exclude_chat_ids, "exclude")] {
                    for room_id in ids {
                        sqlx::query(
                            "INSERT INTO chat_folder_chats (folder_id, room_id, mode) VALUES ($1, $2, $3) \
                             ON CONFLICT (folder_id, room_id) DO NOTHING",
                        )
                        .bind(folder_id)
                        .bind(room_id)
                        .bind(mode)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                tx.commit().await
            }
            .await
        })
    }
}
