//! Runtime-selectable AI endpoints. Secrets remain in environment variables.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{AiConfig, AiRuntimeStatus};
use crate::state::{with_pool, AppState};

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiModelChoice {
    pub id: Uuid,
    pub label: String,
    pub provider: String,
    pub model: String,
    pub fast_model: Option<String>,
    pub ready: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiModelOptionView {
    pub id: Uuid,
    pub label: String,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub enabled: bool,
    pub ready: bool,
    pub source: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SaveAiModelOption {
    pub label: String,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_env: String,
    pub enabled: bool,
}

#[derive(Clone, sqlx::FromRow)]
struct StoredAiModelOption {
    id: Uuid,
    label: String,
    provider: String,
    base_url: String,
    model: String,
    api_key_env: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub(crate) struct ResolvedAiModel {
    pub id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
}

impl AppState {
    pub async fn ai_model_options(&self) -> Result<Vec<AiModelOptionView>, sqlx::Error> {
        let stored: Vec<StoredAiModelOption> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, label, provider, base_url, model, api_key_env, enabled, \
                 created_at, updated_at FROM ai_model_options ORDER BY label ASC, id ASC",
            )
            .fetch_all(pool)
            .await
        })?;
        let mut options = vec![environment_option(&self.config.ai)];
        options.extend(stored.into_iter().map(stored_view));
        Ok(options)
    }

    pub async fn ai_model_choices(&self) -> Result<Vec<AiModelChoice>, sqlx::Error> {
        Ok(self
            .ai_model_options()
            .await?
            .into_iter()
            .filter(|option| option.enabled)
            .map(|option| AiModelChoice {
                id: option.id,
                label: option.label,
                provider: option.provider,
                model: option.model,
                fast_model: (option.source == "environment")
                    .then(|| self.config.ai.fast_model.clone())
                    .flatten(),
                ready: option.ready,
            })
            .collect())
    }

    pub(crate) async fn resolve_ai_model(
        &self,
        requested_id: Option<Uuid>,
        thinking_enabled: bool,
    ) -> Result<Option<ResolvedAiModel>, sqlx::Error> {
        let id = requested_id.filter(|id| !id.is_nil());
        let option = match id {
            Some(id) => self
                .ai_model_options()
                .await?
                .into_iter()
                .find(|option| option.id == id && option.enabled),
            None => Some(environment_option(&self.config.ai)),
        };
        Ok(option.and_then(|option| resolve_option(option, &self.config.ai, id, thinking_enabled)))
    }

    pub async fn create_ai_model_option(
        &self,
        input: &SaveAiModelOption,
    ) -> Result<AiModelOptionView, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO ai_model_options \
                 (id, label, provider, base_url, model, api_key_env, enabled, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)",
            )
            .bind(id)
            .bind(input.label.trim())
            .bind(input.provider.trim())
            .bind(input.base_url.trim_end_matches('/'))
            .bind(input.model.trim())
            .bind(input.api_key_env.trim())
            .bind(input.enabled)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        self.ai_model_option(id)
            .await?
            .map(stored_view)
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn update_ai_model_option(
        &self,
        id: Uuid,
        input: &SaveAiModelOption,
    ) -> Result<Option<AiModelOptionView>, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE ai_model_options SET label = $1, provider = $2, base_url = $3, \
                 model = $4, api_key_env = $5, enabled = $6, updated_at = $7 WHERE id = $8",
            )
            .bind(input.label.trim())
            .bind(input.provider.trim())
            .bind(input.base_url.trim_end_matches('/'))
            .bind(input.model.trim())
            .bind(input.api_key_env.trim())
            .bind(input.enabled)
            .bind(Utc::now())
            .bind(id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() == 1)
        })?;
        if !changed {
            return Ok(None);
        }
        Ok(self.ai_model_option(id).await?.map(stored_view))
    }

    pub async fn delete_ai_model_option(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM ai_model_options WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() == 1)
        })
    }

    async fn ai_model_option(&self, id: Uuid) -> Result<Option<StoredAiModelOption>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, label, provider, base_url, model, api_key_env, enabled, \
                 created_at, updated_at FROM ai_model_options WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await
        })
    }
}

pub fn validate_model_option(input: &SaveAiModelOption) -> bool {
    let valid_provider = matches!(input.provider.trim(), "openai" | "anthropic");
    let valid_url = input.base_url.starts_with("http://") || input.base_url.starts_with("https://");
    valid_provider
        && valid_url
        && valid_text(&input.label, 80)
        && valid_text(&input.model, 160)
        && valid_text(&input.api_key_env, 120)
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}

fn environment_option(config: &AiConfig) -> AiModelOptionView {
    AiModelOptionView {
        id: Uuid::nil(),
        label: "默认环境配置".into(),
        provider: config.provider.clone(),
        base_url: config.base_url.clone().unwrap_or_default(),
        model: config.model.clone(),
        api_key_env: config.api_key_env.clone(),
        enabled: config.enabled,
        ready: config.runtime_status() == AiRuntimeStatus::Ready,
        source: "environment".into(),
        created_at: None,
        updated_at: None,
    }
}

fn stored_view(option: StoredAiModelOption) -> AiModelOptionView {
    let ready = option.enabled
        && std::env::var(&option.api_key_env).is_ok_and(|value| !value.trim().is_empty());
    AiModelOptionView {
        id: option.id,
        label: option.label,
        provider: option.provider,
        base_url: option.base_url,
        model: option.model,
        api_key_env: option.api_key_env,
        enabled: option.enabled,
        ready,
        source: "database".into(),
        created_at: Some(option.created_at),
        updated_at: Some(option.updated_at),
    }
}

fn resolve_option(
    option: AiModelOptionView,
    defaults: &AiConfig,
    id: Option<Uuid>,
    thinking_enabled: bool,
) -> Option<ResolvedAiModel> {
    if !option.enabled || !option.ready {
        return None;
    }
    let mut config = defaults.clone();
    config.enabled = true;
    config.provider = option.provider.clone();
    let model = if id.is_none() && !thinking_enabled {
        defaults
            .fast_model
            .as_ref()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or(&option.model)
            .clone()
    } else {
        option.model.clone()
    };
    config.model = model.clone();
    config.fast_model = None;
    config.base_url = (!option.base_url.is_empty()).then(|| option.base_url.clone());
    config.api_key_env = option.api_key_env.clone();
    config.resolved_api_key()?;
    Some(ResolvedAiModel {
        id,
        provider: option.provider,
        model,
        base_url: option.base_url,
        api_key_env: option.api_key_env,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_option_uses_the_fast_model_outside_thinking_mode() {
        let config = AiConfig {
            enabled: true,
            provider: "openai".into(),
            api_key_env: "PATH".into(),
            model: "reasoning-model".into(),
            fast_model: Some("fast-model".into()),
            base_url: Some("https://ai.example/v1".into()),
            ..AiConfig::default()
        };

        let fast = resolve_option(environment_option(&config), &config, None, false).unwrap();
        let reasoning = resolve_option(environment_option(&config), &config, None, true).unwrap();

        assert_eq!(fast.model, "fast-model");
        assert_eq!(reasoning.model, "reasoning-model");
    }
}
