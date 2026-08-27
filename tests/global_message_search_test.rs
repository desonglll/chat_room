use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use uuid::Uuid;

mod global_search_support;
use global_search_support::{
    create_direct_chat, create_room, insert_message, register, search, search_after, start_server,
};

#[tokio::test]
async fn global_search_is_authorized_filterable_and_stably_paginated() {
    let server = start_server().await;
    let client = Client::new();
    let viewer = register(&client, &server.base, "global-search-viewer").await;
    let teammate = register(&client, &server.base, "global-search-teammate").await;
    let outsider = register(&client, &server.base, "global-search-outsider").await;
    let primary = create_room(&client, &server.base, &viewer, "Global Search Primary").await;
    let secondary = create_room(&client, &server.base, &viewer, "Global Search Secondary").await;
    let hidden = create_room(&client, &server.base, &outsider, "Global Search Hidden").await;
    let direct = create_direct_chat(&client, &server.base, &viewer, &teammate).await;
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{primary}/join-requests", server.base))
            .bearer_auth(&teammate.token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        client
            .put(format!("{}/api/conversations/{primary}/alias", server.base))
            .bearer_auth(&viewer.token)
            .json(&serde_json::json!({ "alias": "Private Project" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let base_time = DateTime::parse_from_rfc3339("2026-08-01T08:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let before_id = Uuid::from_u128(1);
    let target_id = Uuid::from_u128(2);
    let after_id = Uuid::from_u128(3);
    let image_id = Uuid::from_u128(4);
    let secondary_id = Uuid::from_u128(5);
    let same_time_id = Uuid::from_u128(6);
    let recalled_id = Uuid::from_u128(7);
    let hidden_id = Uuid::from_u128(8);
    insert_message(
        &server.state,
        before_id,
        primary,
        &viewer,
        "context before",
        None,
        base_time - Duration::seconds(1),
    )
    .await;
    insert_message(
        &server.state,
        target_id,
        primary,
        &teammate,
        "needle target",
        None,
        base_time,
    )
    .await;
    insert_message(
        &server.state,
        recalled_id,
        primary,
        &viewer,
        "needle recalled",
        None,
        base_time + Duration::milliseconds(500),
    )
    .await;
    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(recalled_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    insert_message(
        &server.state,
        after_id,
        primary,
        &viewer,
        "context after",
        None,
        base_time + Duration::seconds(1),
    )
    .await;
    let attachment_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO attachments \
         (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, created_at) \
         VALUES (?, ?, ?, ?, 'diagram.png', 'image/png', 12, ?)",
    )
    .bind(attachment_id)
    .bind(Uuid::new_v4())
    .bind(primary)
    .bind(viewer.id)
    .bind(base_time + Duration::seconds(2))
    .execute(server.state.pool())
    .await
    .unwrap();
    insert_message(
        &server.state,
        image_id,
        primary,
        &viewer,
        "needle image",
        Some(attachment_id),
        base_time + Duration::seconds(2),
    )
    .await;
    let shared_time = base_time + Duration::seconds(3);
    insert_message(
        &server.state,
        secondary_id,
        secondary,
        &viewer,
        "needle secondary",
        None,
        shared_time,
    )
    .await;
    insert_message(
        &server.state,
        same_time_id,
        primary,
        &viewer,
        "needle same timestamp",
        None,
        shared_time,
    )
    .await;
    insert_message(
        &server.state,
        hidden_id,
        hidden,
        &outsider,
        "needle hidden",
        None,
        base_time + Duration::seconds(4),
    )
    .await;
    insert_message(
        &server.state,
        Uuid::from_u128(9),
        primary,
        &viewer,
        "50%_literal",
        None,
        base_time + Duration::seconds(5),
    )
    .await;
    let direct_id = Uuid::from_u128(10);
    insert_message(
        &server.state,
        direct_id,
        direct,
        &teammate,
        "direct-only result",
        None,
        base_time + Duration::seconds(6),
    )
    .await;

    let first_page = search(&client, &server.base, &viewer.token, "q=needle&limit=2").await;
    assert_eq!(first_page["items"].as_array().unwrap().len(), 2);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    let second_page = search_after(&client, &server.base, &viewer.token, cursor).await;
    assert!(second_page["next_cursor"].is_null());
    let all_items = first_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second_page["items"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(all_items.len(), 4);
    let ids = all_items
        .iter()
        .map(|item| item["message_id"].as_str().unwrap().to_owned())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 4);
    assert!(!ids.contains(&recalled_id.to_string()));
    assert!(!ids.contains(&hidden_id.to_string()));
    let target = all_items
        .iter()
        .find(|item| item["message_id"] == target_id.to_string())
        .unwrap();
    assert_eq!(target["conversation_title"], "Private Project");
    assert_eq!(target["context_before"], "context before");
    assert_eq!(target["context_after"], "context after");

    let image = search(
        &client,
        &server.base,
        &viewer.token,
        "q=needle&content_type=image",
    )
    .await;
    assert_eq!(image["items"].as_array().unwrap().len(), 1);
    assert_eq!(image["items"][0]["message_id"], image_id.to_string());
    assert_eq!(image["items"][0]["attachment_file_name"], "diagram.png");
    let sender = search(
        &client,
        &server.base,
        &viewer.token,
        &format!("q=needle&room_id={primary}&sender_id={}", teammate.id),
    )
    .await;
    assert_eq!(sender["items"].as_array().unwrap().len(), 1);
    assert_eq!(sender["items"][0]["message_id"], target_id.to_string());
    let date_range = search(
        &client,
        &server.base,
        &viewer.token,
        "q=needle&from=2026-08-01T08%3A00%3A00Z&to=2026-08-01T08%3A00%3A00Z",
    )
    .await;
    assert_eq!(date_range["items"].as_array().unwrap().len(), 1);
    assert_eq!(date_range["items"][0]["message_id"], target_id.to_string());
    let literal = search(&client, &server.base, &viewer.token, "q=50%25_literal").await;
    assert_eq!(literal["items"].as_array().unwrap().len(), 1);
    let direct_page = search(&client, &server.base, &viewer.token, "q=direct-only").await;
    assert_eq!(direct_page["items"].as_array().unwrap().len(), 1);
    assert_eq!(direct_page["items"][0]["message_id"], direct_id.to_string());
    assert_eq!(direct_page["items"][0]["conversation_kind"], "direct");
    assert_eq!(
        direct_page["items"][0]["conversation_title"],
        "global-search-teammate"
    );
    let outsider_direct = search(&client, &server.base, &outsider.token, "q=direct-only").await;
    assert!(outsider_direct["items"].as_array().unwrap().is_empty());
    let outsider_page = search(&client, &server.base, &outsider.token, "q=target").await;
    assert!(outsider_page["items"].as_array().unwrap().is_empty());

    assert_eq!(
        client
            .delete(format!("{}/api/rooms/{secondary}/members/me", server.base))
            .bearer_auth(&viewer.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let after_leave = search(&client, &server.base, &viewer.token, "q=secondary").await;
    assert!(after_leave["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn global_search_validates_bounds_and_uses_the_visible_message_index() {
    let server = start_server().await;
    let client = Client::new();
    let viewer = register(&client, &server.base, "global-search-validation").await;
    for query in [
        "q=%20%20",
        "q=valid&limit=0",
        "q=valid&limit=51",
        "q=valid&cursor=invalid",
        "q=valid&content_type=unknown",
        "q=valid&from=2026-08-02T00%3A00%3A00Z&to=2026-08-01T00%3A00%3A00Z",
    ] {
        assert_eq!(
            client
                .get(format!("{}/api/messages/search?{query}", server.base))
                .bearer_auth(&viewer.token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST,
            "query should be rejected: {query}"
        );
    }
    assert_eq!(
        client
            .get(format!("{}/api/messages/search", server.base))
            .bearer_auth(&viewer.token)
            .query(&[("q", "x".repeat(201))])
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        client
            .get(format!("{}/api/messages/search?q=valid", server.base))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let plan: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT id FROM messages \
         WHERE room_id = ? AND recalled_at IS NULL \
         ORDER BY created_at DESC, id DESC LIMIT 30",
    )
    .bind(Uuid::new_v4())
    .fetch_all(server.state.pool())
    .await
    .unwrap();
    assert!(
        plan.iter()
            .any(|(_, _, _, detail)| detail.contains("messages_visible_search_idx")),
        "query plan should use the global search index: {plan:?}"
    );
}
