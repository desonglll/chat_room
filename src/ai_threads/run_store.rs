use chrono::{Duration, Utc};
use uuid::Uuid;

use super::models::{AiRun, AiThreadMessage};
use crate::{
    ai::{model_options::ResolvedAiModel, AiAssistant, AiConfig},
    state::{with_pool, AppState},
};

const RUN_COLUMNS: &str = "id, thread_id, user_message_id, assistant_message_id, \
    client_request_id, room_id, model_option_id, provider, model, status, \
    context_message_count, retrieved_message_count, error_message, created_at, updated_at";

pub(super) enum CreateRunOutcome {
    Created(AiRun),
    Existing(AiRun),
    Busy,
}

impl AppState {
    pub(super) async fn create_ai_run(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        question: &str,
        room_id: Option<Uuid>,
        client_request_id: Uuid,
        selected_model: &ResolvedAiModel,
    ) -> Result<CreateRunOutcome, sqlx::Error> {
        if let Some(run) = self.ai_run_by_request(user_id, client_request_id).await? {
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
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO ai_thread_messages \
                 (id, thread_id, role, content, room_id, context_message_count, status, revision, created_at, updated_at) \
                 VALUES ($1, $2, 'user', $3, $4, NULL, 'completed', 0, $5, $5), \
                        ($6, $2, 'assistant', '', $4, NULL, 'pending', 0, $5, $5)",
            )
            .bind(user_message_id)
            .bind(thread_id)
            .bind(question)
            .bind(room_id)
            .bind(now)
            .bind(assistant_message_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO ai_runs \
                 (id, thread_id, user_id, user_message_id, assistant_message_id, client_request_id, \
                  room_id, model_option_id, provider, model, base_url, api_key_env, status, \
                  attempt_count, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'queued', 0, $13, $13)",
            )
            .bind(run_id)
            .bind(thread_id)
            .bind(user_id)
            .bind(user_message_id)
            .bind(assistant_message_id)
            .bind(client_request_id)
            .bind(room_id)
            .bind(selected_model.id)
            .bind(&selected_model.provider)
            .bind(&selected_model.model)
            .bind(&selected_model.base_url)
            .bind(&selected_model.api_key_env)
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

    pub async fn ai_run(&self, user_id: Uuid, run_id: Uuid) -> Result<Option<AiRun>, sqlx::Error> {
        let query = format!("SELECT {RUN_COLUMNS} FROM ai_runs WHERE id = $1 AND user_id = $2");
        with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(run_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await
        })
    }

    pub(super) async fn ai_run_message(
        &self,
        user_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<AiThreadMessage>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT m.id, m.thread_id, m.role, m.content, m.room_id, \
                 m.context_message_count, m.retrieved_message_count, m.status, m.revision, m.created_at, \
                 COALESCE(m.updated_at, m.created_at) AS updated_at \
                 FROM ai_thread_messages m JOIN ai_runs r ON r.assistant_message_id = m.id \
                 WHERE r.id = $1 AND r.user_id = $2",
            )
            .bind(run_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    async fn ai_run_by_request(
        &self,
        user_id: Uuid,
        client_request_id: Uuid,
    ) -> Result<Option<AiRun>, sqlx::Error> {
        let query = format!(
            "SELECT {RUN_COLUMNS} FROM ai_runs WHERE user_id = $1 AND client_request_id = $2"
        );
        with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(user_id)
                .bind(client_request_id)
                .fetch_optional(pool)
                .await
        })
    }

    async fn active_ai_run(&self, thread_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM ai_runs WHERE thread_id = $1 AND status IN ('queued', 'running') LIMIT 1",
            )
            .bind(thread_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn dispatchable_ai_runs(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM ai_runs WHERE status = 'queued' \
                 OR (status = 'running' AND lease_expires_at < $1) \
                 ORDER BY created_at ASC LIMIT 20",
            )
            .bind(now)
            .fetch_all(pool)
            .await
        })
    }

    pub(super) async fn claim_ai_run(&self, run_id: Uuid) -> Result<bool, sqlx::Error> {
        let now = Utc::now();
        let lease = now + Duration::seconds(60);
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE ai_runs SET status = 'running', attempt_count = attempt_count + 1, \
                 lease_expires_at = $1, updated_at = $2 WHERE id = $3 AND \
                 (status = 'queued' OR (status = 'running' AND lease_expires_at < $2))",
            )
            .bind(lease)
            .bind(now)
            .bind(run_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() == 1)
        })
    }

    pub(super) async fn ai_run_execution(
        &self,
        run_id: Uuid,
    ) -> Result<Option<AiRunExecution>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT r.id, r.thread_id, r.user_id, r.user_message_id, r.assistant_message_id, \
                 r.room_id, r.provider, r.model, r.base_url, r.api_key_env, \
                 t.thinking_enabled, m.content AS question FROM ai_runs r \
                 JOIN ai_threads t ON t.id = r.thread_id \
                 JOIN ai_thread_messages m ON m.id = r.user_message_id WHERE r.id = $1",
            )
            .bind(run_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn update_ai_run_answer(
        &self,
        execution: &AiRunExecution,
        content: &str,
        context_message_count: i64,
        retrieved_message_count: i64,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        let lease = now + Duration::seconds(60);
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE ai_thread_messages SET content = $1, status = 'streaming', \
                 context_message_count = $2, retrieved_message_count = $3, \
                 revision = revision + 1, updated_at = $4 WHERE id = $5",
            )
            .bind(content)
            .bind(context_message_count)
            .bind(retrieved_message_count)
            .bind(now)
            .bind(execution.assistant_message_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE ai_runs SET context_message_count = $1, retrieved_message_count = $2, \
                 lease_expires_at = $3, updated_at = $4 WHERE id = $5 AND status = 'running'",
            )
            .bind(context_message_count)
            .bind(retrieved_message_count)
            .bind(lease)
            .bind(now)
            .bind(execution.id)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await
        })
    }

    pub(super) async fn heartbeat_ai_run(&self, run_id: Uuid) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        let lease = now + Duration::seconds(60);
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE ai_runs SET lease_expires_at = $1, updated_at = $2 \
                 WHERE id = $3 AND status = 'running'",
            )
            .bind(lease)
            .bind(now)
            .bind(run_id)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    pub(super) async fn persisted_ai_run_answer(
        &self,
        message_id: Uuid,
    ) -> Result<String, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT content FROM ai_thread_messages WHERE id = $1")
                .bind(message_id)
                .fetch_one(pool)
                .await
        })
    }

    pub(super) async fn finish_ai_run(
        &self,
        execution: &AiRunExecution,
        content: &str,
        context_message_count: i64,
        retrieved_message_count: i64,
    ) -> Result<(), sqlx::Error> {
        self.set_ai_run_terminal(
            execution,
            "completed",
            content,
            context_message_count,
            retrieved_message_count,
            None,
        )
        .await
    }

    pub(super) async fn fail_ai_run(
        &self,
        execution: &AiRunExecution,
        content: &str,
        error_message: &str,
    ) -> Result<(), sqlx::Error> {
        self.set_ai_run_terminal(execution, "failed", content, 0, 0, Some(error_message))
            .await
    }

    async fn set_ai_run_terminal(
        &self,
        execution: &AiRunExecution,
        status: &str,
        content: &str,
        context_message_count: i64,
        retrieved_message_count: i64,
        error_message: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE ai_thread_messages SET content = $1, status = $2, context_message_count = $3, \
                 retrieved_message_count = $4, revision = revision + 1, updated_at = $5 WHERE id = $6",
            )
            .bind(content)
            .bind(if status == "completed" { "completed" } else { "failed" })
            .bind(context_message_count)
            .bind(retrieved_message_count)
            .bind(now)
            .bind(execution.assistant_message_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE ai_runs SET status = $1, context_message_count = $2, retrieved_message_count = $3, \
                 error_message = $4, lease_expires_at = NULL, updated_at = $5 WHERE id = $6",
            )
            .bind(status)
            .bind(context_message_count)
            .bind(retrieved_message_count)
            .bind(error_message)
            .bind(now)
            .bind(execution.id)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await
        })
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct AiRunExecution {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Uuid,
    pub user_message_id: Uuid,
    pub assistant_message_id: Uuid,
    pub room_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    pub thinking_enabled: bool,
    pub question: String,
}

impl AiRunExecution {
    pub(super) fn assistant(&self, defaults: &AiConfig) -> Option<AiAssistant> {
        let mut config = defaults.clone();
        if !self.provider.is_empty() {
            config.provider = self.provider.clone();
            config.model = self.model.clone();
            config.fast_model = None;
            config.base_url = (!self.base_url.is_empty()).then(|| self.base_url.clone());
            config.api_key_env = self.api_key_env.clone();
        }
        config
            .resolved_api_key()
            .map(|key| AiAssistant::new(&config, key))
    }

    pub(super) fn excludes(&self, message: &AiThreadMessage) -> bool {
        message.id == self.user_message_id || message.id == self.assistant_message_id
    }
}
