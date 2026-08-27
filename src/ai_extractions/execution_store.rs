use chrono::{DateTime, Duration, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    ai::{AiAssistant, AiConfig},
    state::{with_pool, AppState},
};

#[derive(Debug, FromRow)]
pub(super) struct ExtractionMessage {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub attachment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct ExtractionExecution {
    pub id: Uuid,
    pub user_id: Uuid,
    pub room_id: Uuid,
    pub room_name: String,
    pub from_at: DateTime<Utc>,
    pub to_at: DateTime<Utc>,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    pub admission_id: Option<Uuid>,
}

impl ExtractionExecution {
    pub(super) fn assistant(&self, defaults: &AiConfig) -> Option<AiAssistant> {
        let mut config = defaults.clone();
        config.provider = self.provider.clone();
        config.model = self.model.clone();
        config.fast_model = None;
        config.base_url = (!self.base_url.is_empty()).then(|| self.base_url.clone());
        config.api_key_env = self.api_key_env.clone();
        config
            .resolved_api_key()
            .map(|key| AiAssistant::new(&config, key))
    }
}

pub(super) struct ValidatedCandidate {
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub inferred: bool,
    pub dedupe_key: String,
    pub source_ids: Vec<Uuid>,
}

impl AppState {
    pub(super) async fn extraction_messages(
        &self,
        execution: &ExtractionExecution,
    ) -> Result<Vec<ExtractionMessage>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT bounded.id, bounded.sender, bounded.content, bounded.attachment, \
                 bounded.created_at FROM (SELECT messages.id, messages.sender, messages.content, \
                   attachments.file_name AS attachment, messages.created_at \
                 FROM messages LEFT JOIN attachments ON attachments.id = messages.attachment_id \
                 WHERE messages.room_id = $1 AND messages.recalled_at IS NULL \
                   AND messages.created_at >= $2 AND messages.created_at <= $3 \
                   AND EXISTS (SELECT 1 FROM room_memberships memberships JOIN rooms \
                     ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
                     WHERE memberships.room_id = $1 AND memberships.user_id = $4 \
                       AND memberships.status = 'active') \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT 500) bounded \
                 ORDER BY bounded.created_at, bounded.id",
            )
            .bind(execution.room_id)
            .bind(execution.from_at)
            .bind(execution.to_at)
            .bind(execution.user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub(super) async fn dispatchable_extraction_runs(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM ai_extraction_runs WHERE status = 'queued' OR \
                 (status = 'running' AND lease_expires_at < $1) ORDER BY created_at LIMIT 20",
            )
            .bind(Utc::now())
            .fetch_all(pool)
            .await
        })
    }

    pub(super) async fn claim_extraction_run(&self, run_id: Uuid) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE ai_extraction_runs SET status = 'running', attempt_count = attempt_count + 1, \
                 lease_expires_at = $1, error_message = NULL, updated_at = $2 WHERE id = $3 AND \
                 (status = 'queued' OR (status = 'running' AND lease_expires_at < $2))",
            )
            .bind(now + Duration::seconds(90))
            .bind(now)
            .bind(run_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() == 1)
        })
    }

    pub(super) async fn extraction_execution(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ExtractionExecution>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT runs.id, runs.user_id, runs.room_id, rooms.name AS room_name, \
                 runs.from_at, runs.to_at, runs.provider, runs.model, runs.base_url, runs.api_key_env, \
                 runs.admission_id \
                 FROM ai_extraction_runs runs JOIN rooms ON rooms.id = runs.room_id \
                 WHERE runs.id = $1 AND rooms.deleted_at IS NULL",
            )
            .bind(run_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn complete_extraction_run(
        &self,
        execution: &ExtractionExecution,
        message_count: i64,
        candidates: &[ValidatedCandidate],
        input_tokens: i64,
        output_tokens: i64,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let active: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM room_memberships WHERE room_id = $1 AND user_id = $2 \
                 AND status = 'active'",
            )
            .bind(execution.room_id)
            .bind(execution.user_id)
            .fetch_optional(&mut *transaction)
            .await?;
            if active.is_none() {
                return Err(sqlx::Error::RowNotFound);
            }
            for (ordinal, candidate) in candidates.iter().enumerate() {
                let candidate_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO ai_extraction_candidates \
                     (id, user_id, room_id, kind, title, detail, inferred, dedupe_key, status, \
                      created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, \
                      'proposed', $9, $9) ON CONFLICT (user_id, room_id, dedupe_key) DO NOTHING",
                )
                .bind(candidate_id)
                .bind(execution.user_id)
                .bind(execution.room_id)
                .bind(&candidate.kind)
                .bind(&candidate.title)
                .bind(&candidate.detail)
                .bind(candidate.inferred)
                .bind(&candidate.dedupe_key)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
                let stored_id: Uuid = sqlx::query_scalar(
                    "SELECT id FROM ai_extraction_candidates WHERE user_id = $1 AND room_id = $2 \
                     AND dedupe_key = $3",
                )
                .bind(execution.user_id)
                .bind(execution.room_id)
                .bind(&candidate.dedupe_key)
                .fetch_one(&mut *transaction)
                .await?;
                for (source_ordinal, message_id) in candidate.source_ids.iter().enumerate() {
                    let inserted = sqlx::query(
                        "INSERT INTO ai_extraction_candidate_sources \
                         (candidate_id, message_id, ordinal) \
                         SELECT $1, messages.id, $2 FROM messages WHERE messages.id = $3 \
                           AND messages.room_id = $4 AND messages.recalled_at IS NULL \
                           AND EXISTS (SELECT 1 FROM room_memberships WHERE room_id = $4 \
                             AND user_id = $5 AND status = 'active') ON CONFLICT DO NOTHING",
                    )
                    .bind(stored_id)
                    .bind(source_ordinal as i64)
                    .bind(message_id)
                    .bind(execution.room_id)
                    .bind(execution.user_id)
                    .execute(&mut *transaction)
                    .await?;
                    if inserted.rows_affected() == 0 {
                        let exists: Option<i64> = sqlx::query_scalar(
                            "SELECT 1 FROM ai_extraction_candidate_sources sources \
                             JOIN messages ON messages.id = sources.message_id \
                               AND messages.room_id = $3 AND messages.recalled_at IS NULL \
                             WHERE sources.candidate_id = $1 AND sources.message_id = $2 \
                               AND EXISTS (SELECT 1 FROM room_memberships WHERE room_id = $3 \
                                 AND user_id = $4 AND status = 'active')",
                        )
                        .bind(stored_id)
                        .bind(message_id)
                        .bind(execution.room_id)
                        .bind(execution.user_id)
                        .fetch_optional(&mut *transaction)
                        .await?;
                        if exists.is_none() {
                            return Err(sqlx::Error::RowNotFound);
                        }
                    }
                }
                sqlx::query(
                    "INSERT INTO ai_extraction_run_candidates (run_id, candidate_id, ordinal) \
                     VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                )
                .bind(execution.id)
                .bind(stored_id)
                .bind(ordinal as i64)
                .execute(&mut *transaction)
                .await?;
            }
            let completed = sqlx::query(
                "UPDATE ai_extraction_runs SET status = 'completed', message_count = $1, \
                 lease_expires_at = NULL, updated_at = $2 WHERE id = $3 AND status = 'running'",
            )
            .bind(message_count)
            .bind(now)
            .bind(execution.id)
            .execute(&mut *transaction)
            .await?;
            if completed.rows_affected() != 1 {
                transaction.rollback().await?;
                return Err(sqlx::Error::RowNotFound);
            }
            transaction.commit().await
        })?;
        if let Some(admission_id) = execution.admission_id {
            if let Err(error) = self
                .finish_ai_admission(admission_id, "completed", Some(input_tokens), output_tokens)
                .await
            {
                tracing::error!(run_id = %execution.id, "record AI extraction usage failed: {error}");
            }
        }
        Ok(())
    }

    pub(super) async fn fail_extraction_run(
        &self,
        run_id: Uuid,
        message: &str,
    ) -> Result<(), sqlx::Error> {
        let admission_id: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT admission_id FROM ai_extraction_runs WHERE id = $1")
                .bind(run_id)
                .fetch_optional(pool)
                .await
        })?
        .flatten();
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE ai_extraction_runs SET status = 'failed', error_message = $1, \
                 lease_expires_at = NULL, updated_at = $2 WHERE id = $3 AND status = 'running'",
            )
            .bind(message)
            .bind(Utc::now())
            .bind(run_id)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        if let Some(admission_id) = admission_id {
            if let Err(error) = self
                .finish_ai_admission(admission_id, "failed", None, 0)
                .await
            {
                tracing::error!(%run_id, "record failed AI extraction usage failed: {error}");
            }
        }
        Ok(())
    }
}
