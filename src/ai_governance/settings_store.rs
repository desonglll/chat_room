use std::collections::{HashMap, HashSet};

use chrono::Utc;
use uuid::Uuid;

use super::models::{
    AiGovernanceSettings, AiGovernedModel, GovernanceModelRow, GovernanceSettingsRow,
    UpdateAiGovernanceSettings,
};
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn ai_governance_settings(&self) -> Result<AiGovernanceSettings, sqlx::Error> {
        let settings = self.ai_governance_settings_row().await?;
        let rules: Vec<GovernanceModelRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT model_option_id, allowed, input_price_micros_per_million, \
                 output_price_micros_per_million FROM ai_governance_models",
            )
            .fetch_all(pool)
            .await
        })?;
        let by_id: HashMap<_, _> = rules
            .into_iter()
            .map(|rule| (rule.model_option_id, rule))
            .collect();
        let models = self
            .ai_model_options()
            .await?
            .into_iter()
            .map(|model| {
                let rule = by_id.get(&model.id);
                AiGovernedModel {
                    id: model.id,
                    label: model.label,
                    provider: model.provider,
                    model: model.model,
                    ready: model.ready,
                    allowed: rule
                        .map(|rule| rule.allowed)
                        .unwrap_or(!settings.allowlist_enabled),
                    input_price_micros_per_million: rule
                        .map_or(0, |rule| rule.input_price_micros_per_million),
                    output_price_micros_per_million: rule
                        .map_or(0, |rule| rule.output_price_micros_per_million),
                }
            })
            .collect();
        Ok(AiGovernanceSettings {
            max_concurrent_runs: settings.max_concurrent_runs,
            daily_user_token_limit: settings.daily_user_token_limit,
            daily_room_token_limit: settings.daily_room_token_limit,
            allowlist_enabled: settings.allowlist_enabled,
            models,
            updated_at: settings.updated_at,
        })
    }

    pub async fn save_ai_governance_settings(
        &self,
        actor_id: Uuid,
        input: &UpdateAiGovernanceSettings,
    ) -> Result<bool, sqlx::Error> {
        if !valid_settings(input) {
            return Ok(false);
        }
        let valid_ids: HashSet<_> = self
            .ai_model_options()
            .await?
            .into_iter()
            .map(|model| model.id)
            .collect();
        let supplied_ids: HashSet<_> = input.models.iter().map(|model| model.id).collect();
        if supplied_ids.len() != input.models.len() || !supplied_ids.is_subset(&valid_ids) {
            return Ok(false);
        }
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE ai_governance_settings SET max_concurrent_runs = $1, \
                 daily_user_token_limit = $2, daily_room_token_limit = $3, \
                 allowlist_enabled = $4, updated_by = $5, updated_at = $6 WHERE id = 1",
            )
            .bind(input.max_concurrent_runs)
            .bind(input.daily_user_token_limit)
            .bind(input.daily_room_token_limit)
            .bind(input.allowlist_enabled)
            .bind(actor_id)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM ai_governance_models")
                .execute(&mut *transaction)
                .await?;
            for model in &input.models {
                sqlx::query(
                    "INSERT INTO ai_governance_models \
                     (model_option_id, allowed, input_price_micros_per_million, \
                      output_price_micros_per_million, updated_at) VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(model.id)
                .bind(model.allowed)
                .bind(model.input_price_micros_per_million)
                .bind(model.output_price_micros_per_million)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await
        })?;
        Ok(true)
    }

    pub(super) async fn ai_governance_settings_row(
        &self,
    ) -> Result<GovernanceSettingsRow, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT max_concurrent_runs, daily_user_token_limit, daily_room_token_limit, \
                 allowlist_enabled, updated_at FROM ai_governance_settings WHERE id = 1",
            )
            .fetch_one(pool)
            .await
        })
    }

    pub(crate) async fn ai_allowlist(&self) -> Result<Option<HashSet<Uuid>>, sqlx::Error> {
        let settings = self.ai_governance_settings_row().await?;
        if !settings.allowlist_enabled {
            return Ok(None);
        }
        let ids = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT model_option_id FROM ai_governance_models WHERE allowed = $1",
            )
            .bind(true)
            .fetch_all(pool)
            .await
        })?;
        Ok(Some(ids.into_iter().collect()))
    }
}

fn valid_settings(input: &UpdateAiGovernanceSettings) -> bool {
    const MAX_TOKEN_LIMIT: i64 = 1_000_000_000_000;
    input.max_concurrent_runs > 0
        && input.max_concurrent_runs <= 1_000
        && [input.daily_user_token_limit, input.daily_room_token_limit]
            .into_iter()
            .flatten()
            .all(|limit| limit > 0 && limit <= MAX_TOKEN_LIMIT)
        && input.models.iter().all(|model| {
            model.input_price_micros_per_million >= 0
                && model.output_price_micros_per_million >= 0
                && model.input_price_micros_per_million <= MAX_TOKEN_LIMIT
                && model.output_price_micros_per_million <= MAX_TOKEN_LIMIT
        })
}
