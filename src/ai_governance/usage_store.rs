use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::models::{AiUsageAggregate, AiUsageAggregateRow, AiUsageReport};
use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
struct AdmissionUsageRow {
    id: Uuid,
    user_id: Uuid,
    room_id: Option<Uuid>,
    feature: String,
    model_option_id: Uuid,
    provider: String,
    model: String,
    reserved_tokens: i64,
    input_price_micros_per_million: i64,
    output_price_micros_per_million: i64,
    created_at: DateTime<Utc>,
}

impl AppState {
    pub async fn finish_ai_admission(
        &self,
        admission_id: Uuid,
        status: &str,
        input_tokens: Option<i64>,
        output_tokens: i64,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let admission: Option<AdmissionUsageRow> = sqlx::query_as(
                "SELECT id, user_id, room_id, feature, model_option_id, provider, model, \
                 reserved_tokens, input_price_micros_per_million, \
                 output_price_micros_per_million, created_at FROM ai_admissions WHERE id = $1",
            )
            .bind(admission_id)
            .fetch_optional(&mut *transaction)
            .await?;
            let Some(admission) = admission else {
                transaction.rollback().await?;
                return Ok(());
            };
            let input_tokens = input_tokens.unwrap_or(admission.reserved_tokens).max(0);
            let output_tokens = output_tokens.max(0);
            let total_tokens = input_tokens.saturating_add(output_tokens);
            let duration_ms = now
                .signed_duration_since(admission.created_at)
                .num_milliseconds()
                .max(0);
            let cost = token_cost(
                input_tokens,
                output_tokens,
                admission.input_price_micros_per_million,
                admission.output_price_micros_per_million,
            );
            sqlx::query(
                "INSERT INTO ai_usage_records \
                 (id, admission_id, user_id, room_id, feature, model_option_id, provider, model, \
                  status, token_source, input_tokens, output_tokens, total_tokens, duration_ms, \
                  estimated_cost_micros, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'estimated', $10, $11, $12, $13, $14, $15) \
                 ON CONFLICT (admission_id) DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(admission.id)
            .bind(admission.user_id)
            .bind(admission.room_id)
            .bind(admission.feature)
            .bind(admission.model_option_id)
            .bind(admission.provider)
            .bind(admission.model)
            .bind(if status == "completed" { "completed" } else { "failed" })
            .bind(input_tokens)
            .bind(output_tokens)
            .bind(total_tokens)
            .bind(duration_ms)
            .bind(cost)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM ai_admissions WHERE id = $1")
                .bind(admission_id)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await
        })
    }

    pub async fn ai_usage_report(
        &self,
        group_by: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<AiUsageReport, sqlx::Error> {
        let rows: Vec<AiUsageAggregateRow> = if group_by == "model" {
            with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT model_option_id AS group_id, \
                     provider || ' / ' || model AS label, CAST(COUNT(*) AS BIGINT) AS runs, \
                     CAST(SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS BIGINT) AS completed_runs, \
                     CAST(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS BIGINT) AS failed_runs, \
                     CAST(SUM(input_tokens) AS BIGINT) AS input_tokens, \
                     CAST(SUM(output_tokens) AS BIGINT) AS output_tokens, \
                     CAST(SUM(total_tokens) AS BIGINT) AS total_tokens, \
                     CAST(SUM(duration_ms) AS BIGINT) AS duration_ms, \
                     CAST(SUM(estimated_cost_micros) AS BIGINT) AS estimated_cost_micros \
                     FROM ai_usage_records WHERE created_at >= $1 AND created_at < $2 \
                     GROUP BY model_option_id, provider, model \
                     ORDER BY estimated_cost_micros DESC, total_tokens DESC, model_option_id",
                )
                .bind(from)
                .bind(to)
                .fetch_all(pool)
                .await
            })?
        } else {
            with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT records.room_id AS group_id, \
                     rooms.name AS label, CAST(COUNT(*) AS BIGINT) AS runs, \
                     CAST(SUM(CASE WHEN records.status = 'completed' THEN 1 ELSE 0 END) AS BIGINT) AS completed_runs, \
                     CAST(SUM(CASE WHEN records.status = 'failed' THEN 1 ELSE 0 END) AS BIGINT) AS failed_runs, \
                     CAST(SUM(records.input_tokens) AS BIGINT) AS input_tokens, \
                     CAST(SUM(records.output_tokens) AS BIGINT) AS output_tokens, \
                     CAST(SUM(records.total_tokens) AS BIGINT) AS total_tokens, \
                     CAST(SUM(records.duration_ms) AS BIGINT) AS duration_ms, \
                     CAST(SUM(records.estimated_cost_micros) AS BIGINT) AS estimated_cost_micros \
                     FROM ai_usage_records records LEFT JOIN rooms ON rooms.id = records.room_id \
                     WHERE records.created_at >= $1 AND records.created_at < $2 \
                     GROUP BY records.room_id, rooms.name \
                     ORDER BY estimated_cost_micros DESC, total_tokens DESC, records.room_id",
                )
                .bind(from)
                .bind(to)
                .fetch_all(pool)
                .await
            })?
        };
        let items = rows
            .into_iter()
            .map(|row| {
                let key = row
                    .group_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "personal".into());
                let label = row.label.unwrap_or_else(|| {
                    if key == "personal" {
                        "个人 AI".into()
                    } else {
                        key.clone()
                    }
                });
                AiUsageAggregate {
                    key,
                    label,
                    runs: row.runs,
                    completed_runs: row.completed_runs,
                    failed_runs: row.failed_runs,
                    input_tokens: row.input_tokens,
                    output_tokens: row.output_tokens,
                    total_tokens: row.total_tokens,
                    duration_ms: row.duration_ms,
                    estimated_cost_micros: row.estimated_cost_micros,
                }
            })
            .collect();
        Ok(AiUsageReport {
            group_by: group_by.into(),
            from,
            to,
            token_source: "estimated".into(),
            items,
        })
    }
}

fn token_cost(input_tokens: i64, output_tokens: i64, input_rate: i64, output_rate: i64) -> i64 {
    let cost = i128::from(input_tokens) * i128::from(input_rate)
        + i128::from(output_tokens) * i128::from(output_rate);
    (cost / 1_000_000).clamp(0, i128::from(i64::MAX)) as i64
}

#[cfg(test)]
mod tests {
    use super::token_cost;

    #[test]
    fn cost_uses_per_million_token_rates_without_floating_point() {
        assert_eq!(token_cost(1_000, 500, 2_000_000, 8_000_000), 6_000);
    }
}
