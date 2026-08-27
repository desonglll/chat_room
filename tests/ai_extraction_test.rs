mod ai_extraction_support;

use std::sync::atomic::Ordering;

use ai_extraction_support::*;
use chrono::{Duration, Utc};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use uuid::Uuid;

#[tokio::test]
async fn extraction_validates_sources_dedupes_and_confirms_atomically() {
    let server = start_server().await;
    let client = Client::new();
    let owner = register(&client, &server, "extract-owner").await;
    let outsider = register(&client, &server, "extract-outsider").await;
    let room_id = create_room(&client, &server, &owner).await;
    let now = Utc::now();
    let decision_source = insert_message(
        &server,
        room_id,
        &owner,
        "We approve Friday",
        now - Duration::minutes(30),
        false,
    )
    .await;
    let task_source = insert_message(
        &server,
        room_id,
        &owner,
        "Prepare the release notes",
        now - Duration::minutes(20),
        false,
    )
    .await;
    insert_message(
        &server,
        room_id,
        &owner,
        "recalled private text",
        now - Duration::minutes(10),
        true,
    )
    .await;
    insert_message(
        &server,
        room_id,
        &owner,
        "outside range",
        now - Duration::days(2),
        false,
    )
    .await;

    let accepted = create_extraction(
        &client,
        &server,
        &owner,
        room_id,
        now - Duration::hours(1),
        now + Duration::minutes(1),
    )
    .await;
    let run_id = accepted["id"].as_str().unwrap();
    let second_room = create_room(&client, &server, &owner).await;
    assert_eq!(
        client
            .post(format!(
                "{}/api/rooms/{second_room}/ai/extractions",
                server.base
            ))
            .bearer_auth(&owner.token)
            .json(&serde_json::json!({
                "from_at": now - Duration::hours(1),
                "to_at": now,
                "client_request_id": accepted["client_request_id"]
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        client
            .get(format!("{}/api/ai/extractions/{run_id}", server.base))
            .bearer_auth(&outsider.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let completed = wait_for_terminal(&client, &server, &owner.token, run_id).await;
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["message_count"], 2);
    let candidates = completed["candidates"].as_array().unwrap();
    assert_eq!(
        candidates.len(),
        3,
        "duplicate task candidate should collapse"
    );
    assert_eq!(
        candidates[0]["sources"][0]["message_id"],
        decision_source.to_string()
    );
    assert_eq!(
        candidates[1]["sources"][0]["message_id"],
        task_source.to_string()
    );
    assert_eq!(candidates[2]["inferred"], true);
    let provider_request = server.provider.requests.lock().unwrap()[0].to_string();
    assert!(provider_request.contains("We approve Friday"));
    assert!(provider_request.contains("Prepare the release notes"));
    assert!(!provider_request.contains("recalled private text"));
    assert!(!provider_request.contains("outside range"));

    let decision: Value = mutate_candidate(&client, &server, &owner, &candidates[0], "confirm")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(decision["result_kind"], "favorite");
    let task: Value = mutate_candidate(&client, &server, &owner, &candidates[1], "confirm")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(task["result_kind"], "task");
    let task_row: (
        String,
        Option<Uuid>,
        Option<chrono::DateTime<Utc>>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT status, assignee_id, due_at, source_message_id FROM room_tasks WHERE id = ?",
    )
    .bind(Uuid::parse_str(task["result_id"].as_str().unwrap()).unwrap())
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(task_row, ("open".into(), None, None, Some(task_source)));
    assert_eq!(
        mutate_candidate(&client, &server, &owner, &candidates[1], "confirm")
            .await
            .status(),
        StatusCode::OK
    );
    let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_tasks")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(task_count, 1);
    assert_eq!(
        mutate_candidate(&client, &server, &owner, &candidates[2], "dismiss")
            .await
            .status(),
        StatusCode::OK
    );
    let favorite_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM favorites")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(favorite_count, 1);

    let repeated = create_extraction(
        &client,
        &server,
        &owner,
        room_id,
        now - Duration::hours(1),
        now + Duration::minutes(1),
    )
    .await;
    let repeated = wait_for_terminal(
        &client,
        &server,
        &owner.token,
        repeated["id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(
        repeated["candidates"][0]["id"],
        completed["candidates"][0]["id"]
    );
    assert_eq!(repeated["candidates"][0]["status"], "confirmed");
    assert_eq!(repeated["candidates"][2]["status"], "dismissed");
    assert_eq!(server.provider.calls.load(Ordering::SeqCst), 2);

    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(decision_source)
        .execute(server.state.pool())
        .await
        .unwrap();
    assert_eq!(
        mutate_candidate(&client, &server, &owner, &candidates[0], "confirm")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let redacted: Value = client
        .get(format!(
            "{}/api/ai/extractions/{}",
            server.base,
            repeated["id"].as_str().unwrap()
        ))
        .bearer_auth(&owner.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(redacted["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .all(|candidate| candidate["id"] != candidates[0]["id"]));
}

#[tokio::test]
async fn extraction_rejects_unknown_model_sources_before_persistence() {
    let server = start_server().await;
    let client = Client::new();
    let owner = register(&client, &server, "invalid-source-owner").await;
    let room_id = create_room(&client, &server, &owner).await;
    let now = Utc::now();
    insert_message(
        &server,
        room_id,
        &owner,
        "invalid-source-marker",
        now - Duration::minutes(1),
        false,
    )
    .await;
    let accepted = create_extraction(
        &client,
        &server,
        &owner,
        room_id,
        now - Duration::hours(1),
        now,
    )
    .await;
    let failed = wait_for_terminal(
        &client,
        &server,
        &owner.token,
        accepted["id"].as_str().unwrap(),
    )
    .await;
    assert_eq!(failed["status"], "failed");
    let candidates: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_extraction_candidates")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(candidates, 0);
}
