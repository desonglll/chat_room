use chrono::Utc;
use uuid::Uuid;

use crate::favorites::models::{FavoriteCollaborator, FavoriteItem};
use crate::state::{with_pool, AppState};

pub(crate) enum FavoriteUpdateOutcome {
    Updated(FavoriteItem),
    Conflict,
    NotFound,
}

impl AppState {
    pub(crate) async fn update_favorite(
        &self,
        user_id: Uuid,
        favorite_id: Uuid,
        version: i64,
        title: &str,
        content: &str,
    ) -> Result<FavoriteUpdateOutcome, sqlx::Error> {
        let (updated, affected_rooms): (bool, Vec<Uuid>) = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let updated = sqlx::query(
                "UPDATE favorites SET title = $1, content = $2, version = version + 1, \
                 updated_at = $3 WHERE id = $4 AND version = $5 AND \
                 (user_id = $6 OR EXISTS (SELECT 1 FROM favorite_collaborators \
                   WHERE favorite_collaborators.favorite_id = favorites.id \
                     AND favorite_collaborators.user_id = $6) OR EXISTS \
                  (SELECT 1 FROM room_pins \
                   JOIN messages AS pinned_message ON pinned_message.id = room_pins.message_id \
                   JOIN room_memberships AS pinned_membership ON pinned_membership.room_id = room_pins.room_id \
                     AND pinned_membership.user_id = $6 AND pinned_membership.status = 'active' \
                   WHERE pinned_message.favorite_id = favorites.id))",
            )
            .bind(title)
            .bind(content)
            .bind(Utc::now())
            .bind(favorite_id)
            .bind(version)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected()
                > 0;
            let affected_rooms = if updated {
                sqlx::query(
                    "UPDATE messages SET content = (SELECT CASE \
                       WHEN favorites.kind = 'manual' AND favorites.content = '' THEN favorites.title \
                       ELSE favorites.content END FROM favorites WHERE favorites.id = $1), \
                     edited_at = $2 WHERE favorite_id = $1 AND recalled_at IS NULL",
                )
                .bind(favorite_id)
                .bind(Utc::now())
                .execute(&mut *transaction)
                .await?;
                sqlx::query_scalar("SELECT DISTINCT room_id FROM messages WHERE favorite_id = $1")
                    .bind(favorite_id)
                    .fetch_all(&mut *transaction)
                    .await?
            } else {
                Vec::new()
            };
            transaction.commit().await?;
            Ok::<_, sqlx::Error>((updated, affected_rooms))
        })?;
        if updated {
            for room_id in &affected_rooms {
                self.invalidate_message_cache(*room_id).await;
            }
            return self
                .favorite_by_id(user_id, favorite_id)
                .await?
                .map(FavoriteUpdateOutcome::Updated)
                .ok_or(sqlx::Error::RowNotFound);
        }
        Ok(
            if self.favorite_by_id(user_id, favorite_id).await?.is_some() {
                FavoriteUpdateOutcome::Conflict
            } else {
                FavoriteUpdateOutcome::NotFound
            },
        )
    }

    pub(crate) async fn favorite_collaborators(
        &self,
        user_id: Uuid,
        favorite_id: Uuid,
    ) -> Result<Option<Vec<FavoriteCollaborator>>, sqlx::Error> {
        if self.favorite_by_id(user_id, favorite_id).await?.is_none() {
            return Ok(None);
        }
        let collaborators = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.display_name, \
                 users.avatar_emoji, favorite_collaborators.added_at \
                 FROM favorite_collaborators JOIN users \
                   ON users.id = favorite_collaborators.user_id \
                 WHERE favorite_collaborators.favorite_id = $1 \
                 ORDER BY favorite_collaborators.added_at, users.id",
            )
            .bind(favorite_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(Some(collaborators))
    }

    pub(crate) async fn add_favorite_collaborator(
        &self,
        owner_id: Uuid,
        favorite_id: Uuid,
        collaborator_id: Uuid,
    ) -> Result<Option<FavoriteCollaborator>, sqlx::Error> {
        let (low_id, high_id) = ordered_pair(owner_id, collaborator_id);
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO favorite_collaborators (favorite_id, user_id, added_at) \
                 SELECT $1, $2, $3 WHERE $2 <> $4 \
                   AND EXISTS (SELECT 1 FROM favorites WHERE id = $1 AND user_id = $4) \
                   AND EXISTS (SELECT 1 FROM friendships WHERE user_low_id = $5 \
                     AND user_high_id = $6 AND status = 'accepted') \
                 ON CONFLICT (favorite_id, user_id) DO NOTHING",
            )
            .bind(favorite_id)
            .bind(collaborator_id)
            .bind(Utc::now())
            .bind(owner_id)
            .bind(low_id)
            .bind(high_id)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(self
            .favorite_collaborators(owner_id, favorite_id)
            .await?
            .and_then(|items| {
                items
                    .into_iter()
                    .find(|item| item.user_id == collaborator_id)
            }))
    }

    pub(crate) async fn remove_favorite_collaborator(
        &self,
        requester_id: Uuid,
        favorite_id: Uuid,
        collaborator_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "DELETE FROM favorite_collaborators WHERE favorite_id = $1 AND user_id = $2 \
                 AND ($2 = $3 OR EXISTS (SELECT 1 FROM favorites \
                   WHERE favorites.id = $1 AND favorites.user_id = $3))",
            )
            .bind(favorite_id)
            .bind(collaborator_id)
            .bind(requester_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }
}

fn ordered_pair(left: Uuid, right: Uuid) -> (Uuid, Uuid) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}
