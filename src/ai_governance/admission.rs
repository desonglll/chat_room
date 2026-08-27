use chrono::{Duration, Utc};
use uuid::Uuid;

use super::models::{GovernanceModelRow, GovernanceSettingsRow};
use crate::{
    ai::model_options::ResolvedAiModel,
    state::{with_pool, AppState},
};

const ADMISSION_LEASE_MINUTES: i64 = 60;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AiAdmission {
    pub id: Uuid,
}

pub(crate) struct AiAdmissionRequest<'a> {
    pub user_id: Uuid,
    pub room_id: Option<Uuid>,
    pub feature: &'static str,
    pub model: &'a ResolvedAiModel,
    pub reserved_tokens: i64,
}

#[derive(Debug)]
pub(crate) enum AiGovernanceRejection {
    RoomDisabled,
    AdminsOnly,
    ModelBlocked,
    ConcurrencyLimit,
    UserTokenLimit,
    RoomTokenLimit,
    Database(sqlx::Error),
}

impl AiGovernanceRejection {
    pub(crate) fn status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::RoomDisabled | Self::AdminsOnly => StatusCode::FORBIDDEN,
            Self::ModelBlocked => StatusCode::SERVICE_UNAVAILABLE,
            Self::ConcurrencyLimit | Self::UserTokenLimit | Self::RoomTokenLimit => {
                StatusCode::TOO_MANY_REQUESTS
            }
            Self::Database(error) => {
                tracing::error!("AI admission database operation failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl From<sqlx::Error> for AiGovernanceRejection {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl AppState {
    pub(crate) async fn admit_ai(
        &self,
        request: AiAdmissionRequest<'_>,
    ) -> Result<AiAdmission, AiGovernanceRejection> {
        if let Some(room_id) = request.room_id {
            self.check_room_ai_access(room_id, request.user_id).await?;
        }
        let now = Utc::now();
        let day_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("midnight is valid")
            .and_utc();
        let model_id = request.model.id.unwrap_or(Uuid::nil());
        let admission_id = Uuid::new_v4();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE ai_governance_settings SET admission_revision = admission_revision + 1 \
                 WHERE id = 1",
            )
            .execute(&mut *transaction)
            .await?;
            let settings: GovernanceSettingsRow = sqlx::query_as(
                "SELECT max_concurrent_runs, daily_user_token_limit, daily_room_token_limit, \
                 allowlist_enabled, updated_at FROM ai_governance_settings WHERE id = 1",
            )
            .fetch_one(&mut *transaction)
            .await?;
            let rule: Option<GovernanceModelRow> = sqlx::query_as(
                "SELECT model_option_id, allowed, input_price_micros_per_million, \
                 output_price_micros_per_million FROM ai_governance_models \
                 WHERE model_option_id = $1",
            )
            .bind(model_id)
            .fetch_optional(&mut *transaction)
            .await?;
            if settings.allowlist_enabled && !rule.as_ref().is_some_and(|rule| rule.allowed) {
                transaction.rollback().await?;
                return Err(AiGovernanceRejection::ModelBlocked);
            }
            let active: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM ai_admissions WHERE expires_at > $1")
                    .bind(now)
                    .fetch_one(&mut *transaction)
                    .await?;
            if active >= settings.max_concurrent_runs {
                transaction.rollback().await?;
                return Err(AiGovernanceRejection::ConcurrencyLimit);
            }
            let user_used: i64 = sqlx::query_scalar(
                "SELECT CAST(COALESCE(SUM(total_tokens), 0) AS BIGINT) FROM ai_usage_records \
                 WHERE user_id = $1 AND created_at >= $2",
            )
            .bind(request.user_id)
            .bind(day_start)
            .fetch_one(&mut *transaction)
            .await?;
            let user_reserved: i64 = sqlx::query_scalar(
                "SELECT CAST(COALESCE(SUM(reserved_tokens), 0) AS BIGINT) FROM ai_admissions \
                 WHERE user_id = $1 AND expires_at > $2",
            )
            .bind(request.user_id)
            .bind(now)
            .fetch_one(&mut *transaction)
            .await?;
            let user_tokens = user_used.saturating_add(user_reserved);
            if settings
                .daily_user_token_limit
                .is_some_and(|limit| user_tokens.saturating_add(request.reserved_tokens) > limit)
            {
                transaction.rollback().await?;
                return Err(AiGovernanceRejection::UserTokenLimit);
            }
            if let (Some(room_id), Some(limit)) = (request.room_id, settings.daily_room_token_limit)
            {
                let room_used: i64 = sqlx::query_scalar(
                    "SELECT CAST(COALESCE(SUM(total_tokens), 0) AS BIGINT) FROM ai_usage_records \
                     WHERE room_id = $1 AND created_at >= $2",
                )
                .bind(room_id)
                .bind(day_start)
                .fetch_one(&mut *transaction)
                .await?;
                let room_reserved: i64 = sqlx::query_scalar(
                    "SELECT CAST(COALESCE(SUM(reserved_tokens), 0) AS BIGINT) FROM ai_admissions \
                     WHERE room_id = $1 AND expires_at > $2",
                )
                .bind(room_id)
                .bind(now)
                .fetch_one(&mut *transaction)
                .await?;
                let room_tokens = room_used.saturating_add(room_reserved);
                if room_tokens.saturating_add(request.reserved_tokens) > limit {
                    transaction.rollback().await?;
                    return Err(AiGovernanceRejection::RoomTokenLimit);
                }
            }
            let (input_price, output_price) = rule.map_or((0, 0), |rule| {
                (
                    rule.input_price_micros_per_million,
                    rule.output_price_micros_per_million,
                )
            });
            sqlx::query(
                "INSERT INTO ai_admissions \
                 (id, user_id, room_id, feature, model_option_id, provider, model, reserved_tokens, \
                  input_price_micros_per_million, output_price_micros_per_million, expires_at, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            )
            .bind(admission_id)
            .bind(request.user_id)
            .bind(request.room_id)
            .bind(request.feature)
            .bind(model_id)
            .bind(&request.model.provider)
            .bind(&request.model.model)
            .bind(request.reserved_tokens.max(0))
            .bind(input_price)
            .bind(output_price)
            .bind(now + Duration::minutes(ADMISSION_LEASE_MINUTES))
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, AiGovernanceRejection>(AiAdmission { id: admission_id })
        })
    }

    async fn check_room_ai_access(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AiGovernanceRejection> {
        let identity = self.membership_identity(room_id, user_id).await?;
        let Some((status, role)) = identity else {
            return Err(AiGovernanceRejection::AdminsOnly);
        };
        if status != "active" {
            return Err(AiGovernanceRejection::AdminsOnly);
        }
        match self.room_ai_policy(room_id).await?.mode.as_str() {
            "disabled" => Err(AiGovernanceRejection::RoomDisabled),
            "admins" if !matches!(role.as_str(), "owner" | "admin") => {
                Err(AiGovernanceRejection::AdminsOnly)
            }
            _ => Ok(()),
        }
    }

    pub(crate) async fn discard_ai_admission(&self, id: Uuid) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM ai_admissions WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }
}

pub(crate) fn estimate_tokens(parts: impl IntoIterator<Item = impl AsRef<str>>) -> i64 {
    let chars = parts
        .into_iter()
        .map(|part| part.as_ref().chars().count() as i64)
        .sum::<i64>();
    chars.saturating_add(3) / 4
}
