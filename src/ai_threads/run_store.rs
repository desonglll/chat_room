use chrono::{DateTime, Duration, Utc};
use sqlx::types::Json;
use uuid::Uuid;

use super::models::{AiCitationSource, AiRun, AiRunTraceStep, AiThreadMessage};
use crate::{
    ai::{AiAssistant, AiConfig},
    state::{with_pool, AppState},
};

const RUN_COLUMNS: &str = "id, thread_id, user_message_id, assistant_message_id, \
    client_request_id, room_id, purpose, source_after_message_id, source_through_message_id, \
    source_message_count, model_option_id, provider, model, status, \
    context_message_count, retrieved_message_count, error_message, created_at, updated_at";

impl AppState {
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
                 m.context_message_count, m.retrieved_message_count, m.sources, m.trace, \
                 m.status, m.stage, m.stage_started_at, m.revision, m.created_at, \
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

    pub(super) async fn ai_run_by_request(
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

    pub(super) async fn active_ai_run(&self, thread_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
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
                 r.room_id, r.purpose, r.source_after_message_id, r.source_through_message_id, \
                 r.source_message_count, r.provider, r.model, r.base_url, r.api_key_env, \
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
        answer: &AiAnswerSnapshot<'_>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        let lease = now + Duration::seconds(60);
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE ai_thread_messages SET content = $1, status = 'streaming', stage = $2, \
                 stage_started_at = $3, \
                 context_message_count = $4, retrieved_message_count = $5, \
                 sources = $6, trace = $7, revision = $8, updated_at = $9 WHERE id = $10",
            )
            .bind(answer.content)
            .bind(answer.stage)
            .bind(answer.stage_started_at)
            .bind(answer.context_message_count)
            .bind(answer.retrieved_message_count)
            .bind(Json(answer.sources))
            .bind(Json(answer.trace))
            .bind(answer.revision)
            .bind(now)
            .bind(execution.assistant_message_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE ai_runs SET context_message_count = $1, retrieved_message_count = $2, \
                 lease_expires_at = $3, updated_at = $4 WHERE id = $5 AND status = 'running'",
            )
            .bind(answer.context_message_count)
            .bind(answer.retrieved_message_count)
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

    pub(super) async fn finish_ai_run(
        &self,
        execution: &AiRunExecution,
        content: &str,
        context_message_count: i64,
        retrieved_message_count: i64,
        sources: &[AiCitationSource],
        trace: &[AiRunTraceStep],
    ) -> Result<(), sqlx::Error> {
        self.set_ai_run_terminal(
            execution,
            TerminalAiRun {
                status: "completed",
                content,
                context_message_count,
                retrieved_message_count,
                sources,
                trace,
                error_message: None,
            },
        )
        .await
    }

    pub(super) async fn fail_ai_run(
        &self,
        execution: &AiRunExecution,
        failure: FailedAiRun<'_>,
    ) -> Result<(), sqlx::Error> {
        self.set_ai_run_terminal(
            execution,
            TerminalAiRun {
                status: "failed",
                content: failure.content,
                context_message_count: failure.context_message_count,
                retrieved_message_count: failure.retrieved_message_count,
                sources: failure.sources,
                trace: failure.trace,
                error_message: Some(failure.error_message),
            },
        )
        .await
    }

    async fn set_ai_run_terminal(
        &self,
        execution: &AiRunExecution,
        terminal: TerminalAiRun<'_>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE ai_thread_messages SET content = $1, status = $2, context_message_count = $3, \
                 retrieved_message_count = $4, sources = $5, trace = $6, \
                 stage = $7, stage_started_at = $8, \
                 revision = revision + 1, updated_at = $8 WHERE id = $9",
            )
            .bind(terminal.content)
            .bind(if terminal.status == "completed" {
                "completed"
            } else {
                "failed"
            })
            .bind(terminal.context_message_count)
            .bind(terminal.retrieved_message_count)
            .bind(Json(terminal.sources))
            .bind(Json(terminal.trace))
            .bind(if terminal.status == "completed" {
                "completed"
            } else {
                "failed"
            })
            .bind(now)
            .bind(execution.assistant_message_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE ai_runs SET status = $1, context_message_count = $2, retrieved_message_count = $3, \
                 error_message = $4, lease_expires_at = NULL, updated_at = $5 WHERE id = $6",
            )
            .bind(terminal.status)
            .bind(terminal.context_message_count)
            .bind(terminal.retrieved_message_count)
            .bind(terminal.error_message)
            .bind(now)
            .bind(execution.id)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await
        })
    }
}

pub(super) struct FailedAiRun<'a> {
    pub content: &'a str,
    pub context_message_count: i64,
    pub retrieved_message_count: i64,
    pub sources: &'a [AiCitationSource],
    pub trace: &'a [AiRunTraceStep],
    pub error_message: &'a str,
}

pub(super) struct AiAnswerSnapshot<'a> {
    pub content: &'a str,
    pub context_message_count: i64,
    pub retrieved_message_count: i64,
    pub sources: &'a [AiCitationSource],
    pub trace: &'a [AiRunTraceStep],
    pub stage: &'a str,
    pub stage_started_at: DateTime<Utc>,
    pub revision: i64,
}

struct TerminalAiRun<'a> {
    status: &'a str,
    content: &'a str,
    context_message_count: i64,
    retrieved_message_count: i64,
    sources: &'a [AiCitationSource],
    trace: &'a [AiRunTraceStep],
    error_message: Option<&'a str>,
}

#[derive(sqlx::FromRow)]
pub(super) struct AiRunExecution {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_id: Uuid,
    pub user_message_id: Uuid,
    pub assistant_message_id: Uuid,
    pub room_id: Option<Uuid>,
    pub purpose: String,
    pub source_after_message_id: Option<Uuid>,
    pub source_through_message_id: Option<Uuid>,
    pub source_message_count: Option<i64>,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
    pub thinking_enabled: bool,
    pub question: String,
}

impl AiRunExecution {
    pub(super) fn is_catch_up(&self) -> bool {
        self.purpose == "catch_up"
    }

    pub(super) fn request_label(&self) -> String {
        let host = reqwest::Url::parse(&self.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned));
        match host {
            Some(host) => format!("{} · {} · {host}", self.provider, self.model),
            None => format!("{} · {}", self.provider, self.model),
        }
    }

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
