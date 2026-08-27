use chrono::Utc;
use uuid::Uuid;

use super::models::AiRun;
use crate::{
    ai::model_options::ResolvedAiModel,
    ai_governance::AiAdmission,
    state::{with_pool, AppState},
};

pub(super) enum CreateRunOutcome {
    Created(AiRun),
    Existing(AiRun),
    Busy,
}

pub(super) struct CatchUpRunSource {
    pub after_message_id: Option<Uuid>,
    pub through_message_id: Uuid,
    pub message_count: i64,
}

struct RunInput<'a> {
    purpose: &'static str,
    question: &'a str,
    room_id: Option<Uuid>,
    client_request_id: Uuid,
    source: Option<CatchUpRunSource>,
    model: &'a ResolvedAiModel,
    admission: AiAdmission,
}

pub(super) struct NewAiRun<'a> {
    pub question: &'a str,
    pub room_id: Option<Uuid>,
    pub client_request_id: Uuid,
    pub model: &'a ResolvedAiModel,
    pub admission: AiAdmission,
}

pub(super) struct NewCatchUpRun<'a> {
    pub room_id: Uuid,
    pub client_request_id: Uuid,
    pub source: CatchUpRunSource,
    pub model: &'a ResolvedAiModel,
    pub admission: AiAdmission,
}

impl AppState {
    pub(super) async fn create_ai_run(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        input: NewAiRun<'_>,
    ) -> Result<CreateRunOutcome, sqlx::Error> {
        self.create_run(
            user_id,
            thread_id,
            RunInput {
                purpose: "question",
                question: input.question,
                room_id: input.room_id,
                client_request_id: input.client_request_id,
                source: None,
                model: input.model,
                admission: input.admission,
            },
        )
        .await
    }

    pub(super) async fn create_catch_up_run(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        input: NewCatchUpRun<'_>,
    ) -> Result<CreateRunOutcome, sqlx::Error> {
        self.create_run(
            user_id,
            thread_id,
            RunInput {
                purpose: "catch_up",
                question: "总结我尚未阅读的消息，按主题、已确认决定和待确认问题组织；每项具体结论都标注来源。",
                room_id: Some(input.room_id),
                client_request_id: input.client_request_id,
                source: Some(input.source),
                model: input.model,
                admission: input.admission,
            },
        )
        .await
    }

    async fn create_run(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        input: RunInput<'_>,
    ) -> Result<CreateRunOutcome, sqlx::Error> {
        if let Some(run) = self
            .ai_run_by_request(user_id, input.client_request_id)
            .await?
        {
            return Ok(CreateRunOutcome::Existing(run));
        }
        if self.active_ai_run(thread_id).await?.is_some() {
            return Ok(CreateRunOutcome::Busy);
        }
        if self.ai_thread(user_id, thread_id).await?.is_none() {
            return Err(sqlx::Error::RowNotFound);
        }

        let run_id = Uuid::new_v4();
        let user_message_id = Uuid::new_v4();
        let assistant_message_id = Uuid::new_v4();
        let now = Utc::now();
        let (source_after, source_through, source_count) = input
            .source
            .map(|source| {
                (
                    source.after_message_id,
                    Some(source.through_message_id),
                    Some(source.message_count),
                )
            })
            .unwrap_or((None, None, None));
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO ai_thread_messages \
                 (id, thread_id, role, content, room_id, context_message_count, status, stage, \
                  stage_started_at, revision, created_at, updated_at) \
                 VALUES ($1, $2, 'user', $3, $4, NULL, 'completed', 'completed', $5, 0, $5, $5), \
                        ($6, $2, 'assistant', '', $4, NULL, 'pending', 'queued', $5, 0, $5, $5)",
            )
            .bind(user_message_id)
            .bind(thread_id)
            .bind(input.question)
            .bind(input.room_id)
            .bind(now)
            .bind(assistant_message_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO ai_runs \
                 (id, thread_id, user_id, user_message_id, assistant_message_id, client_request_id, \
                  room_id, purpose, source_after_message_id, source_through_message_id, \
                  source_message_count, model_option_id, provider, model, base_url, api_key_env, \
                  admission_id, status, attempt_count, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, \
                         $16, $17, 'queued', 0, $18, $18)",
            )
            .bind(run_id)
            .bind(thread_id)
            .bind(user_id)
            .bind(user_message_id)
            .bind(assistant_message_id)
            .bind(input.client_request_id)
            .bind(input.room_id)
            .bind(input.purpose)
            .bind(source_after)
            .bind(source_through)
            .bind(source_count)
            .bind(input.model.id)
            .bind(&input.model.provider)
            .bind(&input.model.model)
            .bind(&input.model.base_url)
            .bind(&input.model.api_key_env)
            .bind(input.admission.id)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            sqlx::query("UPDATE ai_threads SET updated_at = $1 WHERE id = $2")
                .bind(now)
                .bind(thread_id)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            Ok::<(), sqlx::Error>(())
        })?;
        self.ai_run(user_id, run_id)
            .await?
            .map(CreateRunOutcome::Created)
            .ok_or(sqlx::Error::RowNotFound)
    }
}
