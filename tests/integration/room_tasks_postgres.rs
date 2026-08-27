use super::*;

#[tokio::test]
async fn postgres_room_tasks_complete_the_http_lifecycle() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_room_tasks_complete_the_http_lifecycle").await
    else {
        return;
    };
    let (db_name, test_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&test_url, &AppConfig::default())
            .await
            .expect("PostgreSQL task schema should migrate"),
    );
    let server = start_server_with_state(state.clone()).await;
    let client = reqwest::Client::new();
    let (room_id, _) = create_room(&server, "pg-room-tasks", None).await;
    let token = session_token(&server, "owner-pg-room-tasks").await;

    let created = client
        .post(format!("{server}/api/rooms/{room_id}/tasks"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "title": "Verify PostgreSQL task lifecycle",
            "assignee_id": null,
            "due_at": null,
            "source_message_id": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let created: serde_json::Value = created.json().await.unwrap();
    let task_id = created["id"].as_str().unwrap();
    assert_eq!(created["version"], 1);

    let listed: Vec<serde_json::Value> = client
        .get(format!("{server}/api/rooms/{room_id}/tasks"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["can_delete"], true);

    let updated = client
        .patch(format!("{server}/api/rooms/{room_id}/tasks/{task_id}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "title": "PostgreSQL task verified",
            "status": "done",
            "assignee_id": null,
            "due_at": null,
            "version": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(updated.status(), 200);
    let updated: serde_json::Value = updated.json().await.unwrap();
    assert_eq!(updated["status"], "done");
    assert_eq!(updated["version"], 2);

    assert_eq!(
        client
            .delete(format!("{server}/api/rooms/{room_id}/tasks/{task_id}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_scratch_database(&admin_pool, &db_name).await;
}
