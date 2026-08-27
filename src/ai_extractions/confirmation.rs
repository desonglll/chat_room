use chrono::Utc;
use sqlx::FromRow;
use uuid::Uuid;

use super::models::AiExtractionCandidate;
use crate::state::{with_pool, AppState};

pub(super) enum CandidateMutation {
    Applied(Box<AiExtractionCandidate>),
    NotFound,
    Conflict,
}

#[derive(FromRow)]
struct CandidateRecord {
    room_id: Uuid,
    kind: String,
    title: String,
    detail: String,
    inferred: bool,
    status: String,
    version: i64,
    actor_name: String,
    source_message_id: Option<Uuid>,
}

impl AppState {
    pub(super) async fn update_extraction_candidate(
        &self,
        user_id: Uuid,
        candidate_id: Uuid,
        action: &str,
        version: i64,
    ) -> Result<CandidateMutation, sqlx::Error> {
        let transaction_result = with_pool!(self, |pool| {
            'mutation: {
                let mut transaction = pool.begin().await?;
                let candidate: Option<CandidateRecord> = sqlx::query_as(
                "SELECT candidates.room_id, candidates.kind, candidates.title, candidates.detail, \
                 candidates.inferred, candidates.status, candidates.version, \
                 COALESCE(NULLIF(users.display_name, ''), users.username) AS actor_name, \
                 (SELECT messages.id FROM ai_extraction_candidate_sources sources \
                    JOIN messages ON messages.id = sources.message_id \
                      AND messages.room_id = candidates.room_id AND messages.recalled_at IS NULL \
                  WHERE sources.candidate_id = candidates.id ORDER BY sources.ordinal LIMIT 1) \
                    AS source_message_id \
                 FROM ai_extraction_candidates candidates JOIN users ON users.id = $2 \
                 JOIN rooms ON rooms.id = candidates.room_id AND rooms.deleted_at IS NULL \
                 WHERE candidates.id = $1 AND candidates.user_id = $2 AND EXISTS \
                 (SELECT 1 FROM room_memberships WHERE room_id = candidates.room_id \
                   AND user_id = $2 AND status = 'active')",
            )
            .bind(candidate_id)
            .bind(user_id)
            .fetch_optional(&mut *transaction)
            .await?;
                let Some(candidate) = candidate else {
                    transaction.rollback().await?;
                    break 'mutation Ok::<_, sqlx::Error>(MutationResult::NotFound);
                };
                if !candidate.inferred && candidate.source_message_id.is_none() {
                    transaction.rollback().await?;
                    break 'mutation Ok(MutationResult::NotFound);
                }
                if (candidate.status == "confirmed" && action == "confirm")
                    || (candidate.status == "dismissed" && action == "dismiss")
                {
                    transaction.commit().await?;
                    break 'mutation Ok(MutationResult::Applied);
                }
                if candidate.status != "proposed" || candidate.version != version {
                    transaction.rollback().await?;
                    break 'mutation Ok(MutationResult::Conflict);
                }
                if action == "dismiss" {
                    let changed = sqlx::query(
                    "UPDATE ai_extraction_candidates SET status = 'dismissed', version = version + 1, \
                     updated_at = $1 WHERE id = $2 AND user_id = $3 AND status = 'proposed' \
                     AND version = $4",
                )
                .bind(Utc::now())
                .bind(candidate_id)
                .bind(user_id)
                .bind(version)
                .execute(&mut *transaction)
                .await?;
                    if changed.rows_affected() != 1 {
                        transaction.rollback().await?;
                        break 'mutation Ok(MutationResult::Conflict);
                    }
                    transaction.commit().await?;
                    break 'mutation Ok(MutationResult::Applied);
                }

                let result_id = Uuid::new_v4();
                let result_kind = if candidate.kind == "task" {
                    "task"
                } else {
                    "favorite"
                };
                let now = Utc::now();
                let reserved = sqlx::query(
                    "UPDATE ai_extraction_candidates SET status = 'confirmed', result_kind = $1, \
                 result_id = $2, version = version + 1, updated_at = $3 \
                 WHERE id = $4 AND user_id = $5 AND status = 'proposed' AND version = $6",
                )
                .bind(result_kind)
                .bind(result_id)
                .bind(now)
                .bind(candidate_id)
                .bind(user_id)
                .bind(version)
                .execute(&mut *transaction)
                .await?;
                if reserved.rows_affected() != 1 {
                    transaction.rollback().await?;
                    break 'mutation Ok(MutationResult::Conflict);
                }

                if candidate.kind == "task" {
                    sqlx::query(
                        "INSERT INTO room_tasks \
                     (id, room_id, title, status, assignee_id, created_by_id, created_by_name, \
                      source_message_id, due_at, version, created_at, updated_at) \
                     VALUES ($1, $2, $3, 'open', NULL, $4, $5, $6, NULL, 1, $7, $7)",
                    )
                    .bind(result_id)
                    .bind(candidate.room_id)
                    .bind(&candidate.title)
                    .bind(user_id)
                    .bind(&candidate.actor_name)
                    .bind(candidate.source_message_id)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
                } else {
                    sqlx::query(
                        "INSERT INTO favorites \
                     (id, user_id, kind, title, content, created_at, updated_at) \
                     VALUES ($1, $2, 'manual', $3, $4, $5, $5)",
                    )
                    .bind(result_id)
                    .bind(user_id)
                    .bind(&candidate.title)
                    .bind(&candidate.detail)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
                }
                transaction.commit().await?;
                Ok(MutationResult::Applied)
            }
        })?;
        match transaction_result {
            MutationResult::NotFound => Ok(CandidateMutation::NotFound),
            MutationResult::Conflict => Ok(CandidateMutation::Conflict),
            MutationResult::Applied => self
                .ai_extraction_candidate(user_id, candidate_id)
                .await?
                .map(Box::new)
                .map(CandidateMutation::Applied)
                .ok_or(sqlx::Error::RowNotFound),
        }
    }
}

enum MutationResult {
    Applied,
    NotFound,
    Conflict,
}
