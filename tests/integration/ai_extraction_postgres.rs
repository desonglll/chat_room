use super::*;
use chrono::{Duration, Utc};

#[tokio::test]
async fn postgres_ai_extraction_runs_and_confirms_results_atomically() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_ai_extraction_runs_and_confirms_results_atomically").await
    else {
        return;
    };
    let (db_name, test_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.provider = "openai".into();
    config.ai.api_key_env = "PATH".into();
    config.ai.model = "unused-empty-range-model".into();
    config.ai.base_url = Some("http://127.0.0.1:1/v1".into());
    let state = Arc::new(
        AppState::open_postgres(&test_url, &config)
            .await
            .expect("PostgreSQL AI extraction schema should migrate"),
    );
    let server = start_server_with_state(state.clone()).await;
    let client = reqwest::Client::new();
    let (room_id, _) = create_room(&server, "pg-ai-extraction", None).await;
    let token = session_token(&server, "owner-pg-ai-extraction").await;
    let room_id = room_id.parse::<uuid::Uuid>().unwrap();
    let user = state
        .session_user(uuid::Uuid::parse_str(&token).unwrap())
        .await
        .unwrap()
        .unwrap();
    let now = Utc::now();

    let accepted: serde_json::Value = client
        .post(format!("{server}/api/rooms/{room_id}/ai/extractions"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "from_at": now - Duration::hours(1),
            "to_at": now,
            "client_request_id": uuid::Uuid::new_v4()
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = accepted["id"].as_str().unwrap();
    for _ in 0..50 {
        let run: serde_json::Value = client
            .get(format!("{server}/api/ai/extractions/{run_id}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if run["status"] == "completed" {
            assert_eq!(run["message_count"], 0);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let message_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
         VALUES ($1, $2, $3, 'pg-ai-owner', 'PostgreSQL source', $4)",
    )
    .bind(message_id)
    .bind(room_id)
    .bind(user.id)
    .bind(now)
    .execute(state.postgres_pool().unwrap())
    .await
    .unwrap();
    let task_candidate = uuid::Uuid::new_v4();
    let decision_candidate = uuid::Uuid::new_v4();
    for (id, kind, title, key, inferred) in [
        (
            task_candidate,
            "task",
            "Verify PostgreSQL extraction",
            "pg-task",
            false,
        ),
        (
            decision_candidate,
            "decision",
            "Use PostgreSQL",
            "pg-decision",
            true,
        ),
    ] {
        sqlx::query(
            "INSERT INTO ai_extraction_candidates \
             (id, user_id, room_id, kind, title, detail, inferred, dedupe_key, status, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, 'confirmed by user only', $6, $7, 'proposed', $8, $8)",
        )
        .bind(id)
        .bind(user.id)
        .bind(room_id)
        .bind(kind)
        .bind(title)
        .bind(inferred)
        .bind(key)
        .bind(now)
        .execute(state.postgres_pool().unwrap())
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO ai_extraction_candidate_sources (candidate_id, message_id, ordinal) \
         VALUES ($1, $2, 0)",
    )
    .bind(task_candidate)
    .bind(message_id)
    .execute(state.postgres_pool().unwrap())
    .await
    .unwrap();

    for candidate_id in [task_candidate, decision_candidate] {
        let response = client
            .patch(format!(
                "{server}/api/ai/extraction-candidates/{candidate_id}"
            ))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "action": "confirm", "version": 1 }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let task: (
        String,
        Option<uuid::Uuid>,
        Option<chrono::DateTime<Utc>>,
        Option<uuid::Uuid>,
    ) = sqlx::query_as(
        "SELECT status, assignee_id, due_at, source_message_id FROM room_tasks \
             WHERE source_message_id = $1",
    )
    .bind(message_id)
    .fetch_one(state.postgres_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(task, ("open".into(), None, None, Some(message_id)));
    let favorites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM favorites WHERE user_id = $1")
        .bind(user.id)
        .fetch_one(state.postgres_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(favorites, 1);

    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_scratch_database(&admin_pool, &db_name).await;
}
