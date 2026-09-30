use chat_room::{
    ai_governance::{UpdateAiGovernanceModel, UpdateAiGovernanceSettings},
    config::AppConfig,
    models::Chat,
    state::AppState,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

mod service_skip;

#[tokio::test]
async fn postgres_governance_policy_settings_and_usage_match_sqlite_contract() {
    let Some((admin_url, admin_pool)) = service_skip::postgres_admin_pool_or_skip(
        "postgres_governance_policy_settings_and_usage_match_sqlite_contract",
    )
    .await
    else {
        return;
    };
    let database_name = format!("chat_room_ai_governance_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database_url = format!(
        "{}/{}",
        admin_url.rsplit_once('/').unwrap().0,
        database_name
    );
    let state = AppState::open_postgres(&database_url, &AppConfig::default())
        .await
        .unwrap();
    let owner = state
        .insert_user("pg-governance-owner", "unused")
        .await
        .unwrap();
    let now = Utc::now();
    let chat = Chat {
        id: Uuid::new_v4(),
        title: "Postgres governance".into(),
        creator_user_id: Some(owner.id),
        membership_status: Some("active".into()),
        membership_role: Some("owner".into()),
        created_at: now,
        ..Chat::default()
    };
    state
        .create_chat_with_owner(chat.clone(), owner.id)
        .await
        .unwrap();
    assert_eq!(state.chat_ai_policy(chat.id).await.unwrap().version, 0);
    let policy = state
        .update_chat_ai_policy(chat.id, owner.id, "admins", 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(policy.mode, "admins");
    assert_eq!(policy.version, 1);
    assert!(state
        .update_chat_ai_policy(chat.id, owner.id, "disabled", 0)
        .await
        .unwrap()
        .is_none());

    let model_id = Uuid::nil();
    let saved = state
        .save_ai_governance_settings(
            owner.id,
            &UpdateAiGovernanceSettings {
                max_concurrent_runs: 3,
                daily_user_token_limit: Some(50_000),
                daily_room_token_limit: Some(100_000),
                allowlist_enabled: true,
                models: vec![UpdateAiGovernanceModel {
                    id: model_id,
                    allowed: true,
                    input_price_micros_per_million: 2_000_000,
                    output_price_micros_per_million: 8_000_000,
                }],
            },
        )
        .await
        .unwrap();
    assert!(saved);
    assert_eq!(
        state
            .ai_governance_settings()
            .await
            .unwrap()
            .max_concurrent_runs,
        3
    );

    let admission_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai_admissions (id, user_id, room_id, feature, model_option_id, provider, model, \
         reserved_tokens, input_price_micros_per_million, output_price_micros_per_million, expires_at, created_at) \
         VALUES ($1, $2, $3, 'question', $4, 'openai', 'pg-test', 0, 2000000, 8000000, $5, $6)",
    )
    .bind(admission_id)
    .bind(owner.id)
    .bind(chat.id)
    .bind(model_id)
    .bind(now + Duration::hours(1))
    .bind(now - Duration::seconds(1))
    .execute(state.postgres_pool().unwrap())
    .await
    .unwrap();
    state
        .finish_ai_admission(admission_id, "completed", Some(1_000), 500)
        .await
        .unwrap();
    let chat_usage = state
        .ai_usage_report("room", now - Duration::days(1), now + Duration::minutes(1))
        .await
        .unwrap();
    assert_eq!(chat_usage.items[0].key, chat.id.to_string());
    assert_eq!(chat_usage.items[0].estimated_cost_micros, 6_000);
    assert_eq!(
        state
            .ai_usage_report("model", now - Duration::days(1), now + Duration::minutes(1))
            .await
            .unwrap()
            .items[0]
            .key,
        model_id.to_string()
    );
    sqlx::query("DELETE FROM chats WHERE id = $1")
        .bind(chat.id)
        .execute(state.postgres_pool().unwrap())
        .await
        .unwrap();
    let retained = state
        .ai_usage_report("room", now - Duration::days(1), now + Duration::minutes(1))
        .await
        .unwrap();
    assert_eq!(retained.items[0].key, chat.id.to_string());
    assert_eq!(retained.items[0].label, chat.id.to_string());

    state.postgres_pool().unwrap().close().await;
    drop(state);
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(&database_name)
        .execute(&admin_pool)
        .await
        .ok();
    sqlx::query(&format!(r#"DROP DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
}
