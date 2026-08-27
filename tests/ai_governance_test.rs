mod ai_governance_support;
mod support;

use ai_governance_support::{
    create_room, create_thread, patch_policy, save_governance, slow_provider, start,
};
use chrono::{Duration, Utc};
use reqwest::{Client, StatusCode};
use support::{session_token, system_admin_token};
use uuid::Uuid;

#[tokio::test]
async fn owners_and_deployment_admins_control_policy_models_limits_and_usage() {
    let (provider_url, provider) = slow_provider().await;
    let server = start(provider_url).await;
    let client = Client::new();
    let owner = system_admin_token(&server.state, &server.base, "governance-owner").await;
    let member = session_token(&server.base, "governance-member").await;
    let room_id = create_room(&client, &server, &owner).await;
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&member)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let default_policy: serde_json::Value = client
        .get(format!("{}/api/rooms/{room_id}/ai-policy", server.base))
        .bearer_auth(&member)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(default_policy["mode"], "members");
    assert_eq!(default_policy["version"], 0);
    assert_eq!(default_policy["applies_to"], "new_runs_only");
    assert_eq!(
        client
            .get(format!("{}/api/admin/ai-governance", server.base))
            .bearer_auth(&member)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        patch_policy(&client, &server, &member, room_id, "admins", 0)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let policy: serde_json::Value = patch_policy(&client, &server, &owner, room_id, "admins", 0)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(policy["version"], 1);
    assert_eq!(
        patch_policy(&client, &server, &owner, room_id, "members", 0)
            .await
            .status(),
        StatusCode::CONFLICT
    );

    let member_room_thread = create_thread(&client, &server, &member, Some(room_id)).await;
    let policy_blocked = client
        .post(format!(
            "{}/api/ai/threads/{member_room_thread}/runs",
            server.base
        ))
        .bearer_auth(&member)
        .json(&serde_json::json!({
            "question": "member room question", "client_request_id": Uuid::new_v4()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(policy_blocked.status(), StatusCode::FORBIDDEN);

    save_governance(&client, &server, &owner, 8, None, false).await;
    let blocked_settings: serde_json::Value = client
        .get(format!("{}/api/admin/ai-governance", server.base))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(blocked_settings["models"][0]["allowed"], false);
    let blocked_models: serde_json::Value = client
        .get(format!("{}/api/ai/models", server.base))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(blocked_models.as_array().unwrap().len(), 0);
    let blocked_model_thread = create_thread(&client, &server, &owner, None).await;
    assert_eq!(
        client
            .post(format!(
                "{}/api/ai/threads/{blocked_model_thread}/runs",
                server.base
            ))
            .bearer_auth(&owner)
            .json(&serde_json::json!({
                "question": "blocked model", "client_request_id": Uuid::new_v4()
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    save_governance(&client, &server, &owner, 8, Some(10), true).await;
    let user = server
        .state
        .session_user(Uuid::parse_str(&member).unwrap())
        .await
        .unwrap()
        .unwrap();
    let admission_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO ai_admissions (id, user_id, room_id, feature, model_option_id, provider, model, \
         reserved_tokens, input_price_micros_per_million, output_price_micros_per_million, expires_at, created_at) \
         VALUES ($1, $2, $3, 'question', $4, 'openai', 'gpt-governance-test', 0, 2000000, 8000000, $5, $6)",
    )
    .bind(admission_id)
    .bind(user.id)
    .bind(room_id)
    .bind(Uuid::nil())
    .bind(now + Duration::hours(1))
    .bind(now - Duration::milliseconds(250))
    .execute(server.state.pool())
    .await
    .unwrap();
    server
        .state
        .finish_ai_admission(admission_id, "completed", Some(1_000), 500)
        .await
        .unwrap();
    server
        .state
        .ai_usage_report("room", now - Duration::days(30), now + Duration::minutes(1))
        .await
        .unwrap();
    let report: serde_json::Value = client
        .get(format!("{}/api/admin/ai-usage?group_by=room", server.base))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(report["token_source"], "estimated");
    assert_eq!(report["items"][0]["total_tokens"], 1_500);
    assert_eq!(report["items"][0]["estimated_cost_micros"], 6_000);
    sqlx::query("DELETE FROM rooms WHERE id = $1")
        .bind(room_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let retained = server
        .state
        .ai_usage_report("room", now - Duration::days(30), now + Duration::minutes(1))
        .await
        .unwrap();
    assert_eq!(retained.items[0].key, room_id.to_string());
    assert_eq!(retained.items[0].label, room_id.to_string());
    let thread_id = create_thread(&client, &server, &member, None).await;
    let limited = client
        .post(format!("{}/api/ai/threads/{thread_id}/runs", server.base))
        .bearer_auth(&member)
        .json(&serde_json::json!({ "question": "quota", "client_request_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    provider.abort();
}

#[tokio::test]
async fn concurrency_is_atomic_and_policy_changes_only_reject_new_runs() {
    let (provider_url, provider) = slow_provider().await;
    let server = start(provider_url).await;
    let client = Client::new();
    let owner = system_admin_token(&server.state, &server.base, "running-policy-owner").await;
    let room_id = create_room(&client, &server, &owner).await;
    save_governance(&client, &server, &owner, 1, None, true).await;
    let thread_id = create_thread(&client, &server, &owner, Some(room_id)).await;
    let client_request_id = Uuid::new_v4();
    let accepted = client
        .post(format!("{}/api/ai/threads/{thread_id}/runs", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({
            "question": "long running", "room_id": room_id, "client_request_id": client_request_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let run: serde_json::Value = accepted.json().await.unwrap();
    let replayed = client
        .post(format!("{}/api/ai/threads/{thread_id}/runs", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({
            "question": "long running", "room_id": room_id, "client_request_id": client_request_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::ACCEPTED);
    assert_eq!(
        replayed.json::<serde_json::Value>().await.unwrap()["id"],
        run["id"]
    );
    let disabled: serde_json::Value =
        patch_policy(&client, &server, &owner, room_id, "disabled", 0)
            .await
            .json()
            .await
            .unwrap();
    assert_eq!(disabled["applies_to"], "new_runs_only");
    let existing: serde_json::Value = client
        .get(format!(
            "{}/api/ai/runs/{}",
            server.base,
            run["id"].as_str().unwrap()
        ))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(matches!(
        existing["status"].as_str(),
        Some("queued" | "running")
    ));
    let blocked_thread = create_thread(&client, &server, &owner, Some(room_id)).await;
    let blocked = client
        .post(format!(
            "{}/api/ai/threads/{blocked_thread}/runs",
            server.base
        ))
        .bearer_auth(&owner)
        .json(
            &serde_json::json!({ "question": "new room run", "client_request_id": Uuid::new_v4() }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    let personal = create_thread(&client, &server, &owner, None).await;
    let concurrent = client
        .post(format!("{}/api/ai/threads/{personal}/runs", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "question": "second", "client_request_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(concurrent.status(), StatusCode::TOO_MANY_REQUESTS);
    provider.abort();
}
