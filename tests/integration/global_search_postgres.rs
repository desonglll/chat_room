use super::*;

#[tokio::test]
async fn postgres_global_search_matches_the_sqlite_contract_and_uses_its_index() {
    let Some((admin_url, admin_pool)) = connect_postgres_admin(
        "postgres_global_search_matches_the_sqlite_contract_and_uses_its_index",
    )
    .await
    else {
        return;
    };
    let (db_name, test_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&test_url, &AppConfig::default())
            .await
            .expect("open postgres global search database"),
    );
    let server = start_server_with_state(state.clone()).await;
    let client = reqwest::Client::new();
    let token = session_token(&server, "pg-global-search").await;
    let room: serde_json::Value = client
        .post(format!("{server}/api/rooms"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "name": "pg-global-search-room" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap();
    let user: serde_json::Value = client
        .get(format!("{server}/api/users/me"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let user_id = user["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap();
    let message_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
         VALUES ($1, $2, $3, 'pg-global-search', 'postgres exact%_needle', $4)",
    )
    .bind(message_id)
    .bind(room_id)
    .bind(user_id)
    .bind(chrono::Utc::now())
    .execute(state.postgres_pool().unwrap())
    .await
    .unwrap();

    let page: serde_json::Value = client
        .get(format!("{server}/api/messages/search"))
        .bearer_auth(&token)
        .query(&[("q", "exact%_needle"), ("content_type", "text")])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["message_id"], message_id.to_string());
    assert_eq!(page["items"][0]["conversation_kind"], "group");

    let pool = state.postgres_pool().unwrap();
    sqlx::query("SET enable_seqscan = off")
        .execute(pool)
        .await
        .unwrap();
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT id FROM messages WHERE room_id = $1 AND recalled_at IS NULL \
         ORDER BY created_at DESC, id DESC LIMIT 30",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(
        plan.iter()
            .any(|line| line.contains("messages_visible_search_idx")),
        "postgres query plan should use the search index: {plan:?}"
    );

    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_scratch_database(&admin_pool, &db_name).await;
}
