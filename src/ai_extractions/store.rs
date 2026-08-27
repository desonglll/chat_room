use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::models::{
    AiExtractionCandidate, AiExtractionRun, AiExtractionSource, ExtractionCandidateRow,
    ExtractionRunRow, ExtractionSourceRow,
};
use crate::{
    ai::model_options::ResolvedAiModel,
    ai_governance::AiAdmission,
    state::{with_pool, AppState},
};

const RUN_COLUMNS: &str = "id, room_id, client_request_id, from_at, to_at, model_option_id, \
    provider, model, status, message_count, error_message, created_at, updated_at";

pub(super) struct NewExtractionRun<'a> {
    pub room_id: Uuid,
    pub from_at: DateTime<Utc>,
    pub to_at: DateTime<Utc>,
    pub client_request_id: Uuid,
    pub model: &'a ResolvedAiModel,
    pub admission: AiAdmission,
}

impl AppState {
    pub(super) async fn create_extraction_run(
        &self,
        user_id: Uuid,
        input: NewExtractionRun<'_>,
    ) -> Result<(AiExtractionRun, bool), sqlx::Error> {
        if let Some(run) = self
            .ai_extraction_run_by_request(user_id, input.client_request_id)
            .await?
        {
            return Ok((run, false));
        }
        let id = Uuid::new_v4();
        let now = Utc::now();
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO ai_extraction_runs \
                 (id, user_id, room_id, client_request_id, from_at, to_at, model_option_id, \
                  provider, model, base_url, api_key_env, admission_id, status, created_at, updated_at) \
                 SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'queued', $13, $13 \
                 WHERE EXISTS (SELECT 1 FROM room_memberships memberships \
                   JOIN rooms ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
                   WHERE memberships.room_id = $3 AND memberships.user_id = $2 \
                     AND memberships.status = 'active') \
                 ON CONFLICT (user_id, client_request_id) DO NOTHING",
            )
            .bind(id)
            .bind(user_id)
            .bind(input.room_id)
            .bind(input.client_request_id)
            .bind(input.from_at)
            .bind(input.to_at)
            .bind(input.model.id)
            .bind(&input.model.provider)
            .bind(&input.model.model)
            .bind(&input.model.base_url)
            .bind(&input.model.api_key_env)
            .bind(input.admission.id)
            .bind(now)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() == 1)
        })?;
        let run = if inserted {
            self.ai_extraction_run(user_id, id).await?
        } else {
            self.ai_extraction_run_by_request(user_id, input.client_request_id)
                .await?
        };
        run.map(|run| (run, inserted))
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub(super) async fn ai_extraction_run(
        &self,
        user_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<AiExtractionRun>, sqlx::Error> {
        let query = format!(
            "SELECT {RUN_COLUMNS} FROM ai_extraction_runs runs \
             WHERE runs.id = $1 AND runs.user_id = $2 AND EXISTS \
             (SELECT 1 FROM room_memberships memberships JOIN rooms \
                ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
              WHERE memberships.room_id = runs.room_id AND memberships.user_id = $2 \
                AND memberships.status = 'active')"
        );
        let row: Option<ExtractionRunRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(run_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await
        })?;
        match row {
            Some(row) => Ok(Some(self.hydrate_extraction_run(user_id, row).await?)),
            None => Ok(None),
        }
    }

    pub(super) async fn ai_extraction_run_by_request(
        &self,
        user_id: Uuid,
        request_id: Uuid,
    ) -> Result<Option<AiExtractionRun>, sqlx::Error> {
        let query = format!(
            "SELECT {RUN_COLUMNS} FROM ai_extraction_runs WHERE user_id = $1 \
             AND client_request_id = $2"
        );
        let row: Option<ExtractionRunRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(user_id)
                .bind(request_id)
                .fetch_optional(pool)
                .await
        })?;
        match row {
            Some(row) => Ok(Some(self.hydrate_extraction_run(user_id, row).await?)),
            None => Ok(None),
        }
    }

    async fn hydrate_extraction_run(
        &self,
        user_id: Uuid,
        row: ExtractionRunRow,
    ) -> Result<AiExtractionRun, sqlx::Error> {
        let candidates: Vec<ExtractionCandidateRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT candidates.id, candidates.kind, candidates.title, candidates.detail, \
                 candidates.inferred, candidates.status, candidates.result_kind, \
                 candidates.result_id, candidates.version, candidates.created_at, \
                 candidates.updated_at FROM ai_extraction_run_candidates links \
                 JOIN ai_extraction_candidates candidates ON candidates.id = links.candidate_id \
                 WHERE links.run_id = $1 ORDER BY links.ordinal, candidates.id",
            )
            .bind(row.id)
            .fetch_all(pool)
            .await
        })?;
        let sources: Vec<ExtractionSourceRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT sources.candidate_id, messages.id AS message_id, messages.sender, \
                 messages.content, messages.created_at AS sent_at \
                 FROM ai_extraction_run_candidates links \
                 JOIN ai_extraction_candidate_sources sources \
                   ON sources.candidate_id = links.candidate_id \
                 JOIN messages ON messages.id = sources.message_id \
                   AND messages.room_id = $2 AND messages.recalled_at IS NULL \
                 JOIN room_memberships memberships ON memberships.room_id = messages.room_id \
                   AND memberships.user_id = $3 AND memberships.status = 'active' \
                 WHERE links.run_id = $1 ORDER BY links.ordinal, sources.ordinal",
            )
            .bind(row.id)
            .bind(row.room_id)
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        let mut by_candidate: HashMap<Uuid, Vec<AiExtractionSource>> = HashMap::new();
        for source in sources {
            by_candidate
                .entry(source.candidate_id)
                .or_default()
                .push(source.into_view());
        }
        Ok(AiExtractionRun {
            id: row.id,
            room_id: row.room_id,
            client_request_id: row.client_request_id,
            from_at: row.from_at,
            to_at: row.to_at,
            model_option_id: row.model_option_id,
            provider: row.provider,
            model: row.model,
            status: row.status,
            message_count: row.message_count,
            error_message: row.error_message,
            candidates: candidates
                .into_iter()
                .filter_map(|candidate| {
                    let sources = by_candidate.remove(&candidate.id).unwrap_or_default();
                    (candidate.inferred || !sources.is_empty())
                        .then(|| candidate.into_view(sources))
                })
                .collect(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub(super) async fn ai_extraction_candidate(
        &self,
        user_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<Option<AiExtractionCandidate>, sqlx::Error> {
        let candidate: Option<ExtractionCandidateRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT candidates.id, candidates.kind, candidates.title, candidates.detail, \
                 candidates.inferred, candidates.status, candidates.result_kind, \
                 candidates.result_id, candidates.version, candidates.created_at, \
                 candidates.updated_at FROM ai_extraction_candidates candidates \
                 WHERE candidates.id = $1 AND candidates.user_id = $2 AND EXISTS \
                 (SELECT 1 FROM room_memberships memberships JOIN rooms \
                    ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
                  WHERE memberships.room_id = candidates.room_id \
                    AND memberships.user_id = $2 AND memberships.status = 'active')",
            )
            .bind(candidate_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let sources: Vec<ExtractionSourceRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT sources.candidate_id, messages.id AS message_id, messages.sender, \
                 messages.content, messages.created_at AS sent_at \
                 FROM ai_extraction_candidate_sources sources \
                 JOIN ai_extraction_candidates candidates ON candidates.id = sources.candidate_id \
                 JOIN messages ON messages.id = sources.message_id \
                   AND messages.room_id = candidates.room_id AND messages.recalled_at IS NULL \
                 WHERE candidates.id = $1 AND candidates.user_id = $2 \
                   AND EXISTS (SELECT 1 FROM room_memberships WHERE room_id = candidates.room_id \
                     AND user_id = $2 AND status = 'active') ORDER BY sources.ordinal",
            )
            .bind(candidate_id)
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        let sources = sources
            .into_iter()
            .map(ExtractionSourceRow::into_view)
            .collect::<Vec<_>>();
        if !candidate.inferred && sources.is_empty() {
            return Ok(None);
        }
        Ok(Some(candidate.into_view(sources)))
    }

    pub(super) async fn extraction_run_room(
        &self,
        user_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT room_id FROM ai_extraction_runs WHERE id = $1 AND user_id = $2",
            )
            .bind(run_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn extraction_candidate_room(
        &self,
        user_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT room_id FROM ai_extraction_candidates WHERE id = $1 AND user_id = $2",
            )
            .bind(candidate_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }
}

impl ExtractionCandidateRow {
    pub(super) fn into_view(self, sources: Vec<AiExtractionSource>) -> AiExtractionCandidate {
        AiExtractionCandidate {
            id: self.id,
            kind: self.kind,
            title: self.title,
            detail: self.detail,
            inferred: self.inferred || sources.is_empty(),
            sources,
            status: self.status,
            result_kind: self.result_kind,
            result_id: self.result_id,
            version: self.version,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl ExtractionSourceRow {
    pub(super) fn into_view(self) -> AiExtractionSource {
        AiExtractionSource {
            message_id: self.message_id,
            sender: self.sender,
            excerpt: truncate(&self.content, 180),
            sent_at: self.sent_at,
        }
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut result = value.chars().take(limit - 1).collect::<String>();
    result.push('…');
    result
}
