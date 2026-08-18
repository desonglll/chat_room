use super::*;

#[tokio::test]
async fn ws_public_chat_works() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "open-chat", None).await;

    let (mut sink_a, mut stream_a) = ws_connect(&base, &id, "alice", None).await;
    let (_sink_b, mut stream_b) = ws_connect(&base, &id, "bob", None).await;

    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("alice"));
    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("bob"));
    let msg = read_until_content(&mut stream_b, "bob").await;
    assert_eq!(msg["type"], "system");

    sink_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "Hi from public room!" }).to_string(),
        ))
        .await
        .unwrap();

    let msg = read_until_type(&mut stream_b, "broadcast").await;
    assert_eq!(msg["sender"], "alice");
    assert_eq!(msg["content"], "Hi from public room!");
}

#[tokio::test]
async fn rooms_survive_sqlite_restart() {
    let database = temp_path("chat-rooms-restart", "db");
    assert!(!database.exists());

    let state1 = Arc::new(AppState::open(&database).await.unwrap());
    assert!(
        database.exists(),
        "database should be created automatically"
    );
    let server1 = start_server_with_state(state1.clone()).await;

    let (private_id, _) = create_room(&server1, "persistent-private", Some("pw")).await;
    let (public_id, _) = create_room(&server1, "persistent-public", None).await;

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rooms")
        .fetch_one(state1.pool())
        .await
        .unwrap();
    assert_eq!(stored, 2);

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(state1.pool())
        .await
        .unwrap();
    assert_eq!(journal_mode, "wal");

    server1.shutdown().await;
    state1.pool().close().await;
    drop(state1);

    let state2 = Arc::new(AppState::open(&database).await.unwrap());
    let server2 = start_server_with_state(state2.clone()).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", server2))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);

    let ids: Vec<&str> = list.iter().filter_map(|room| room["id"].as_str()).collect();
    assert!(ids.contains(&private_id.as_str()));
    assert!(ids.contains(&public_id.as_str()));

    let (_private_sink, _private_stream) =
        ws_connect(&server2, &private_id, "returning-user", Some("pw")).await;
    let (_public_sink, _public_stream) = ws_connect(&server2, &public_id, "guest", None).await;

    server2.shutdown().await;
    state2.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn messages_survive_restart_and_replay_in_order() {
    let database = temp_path("chat-messages-restart", "db");
    let state1 = Arc::new(AppState::open(&database).await.unwrap());
    let server1 = start_server_with_state(state1.clone()).await;
    let (room_id, _) = create_room(&server1, "message-session", None).await;
    let alice_token = session_token(&server1, "alice").await;

    let (mut sink, mut stream) = ws_connect(&server1, &room_id, "alice", None).await;
    assert_eq!(read_json(&mut stream).await["type"], "system");

    let mut message_ids = Vec::new();
    for content in ["first", "second", "third"] {
        sink.send(Message::Text(
            serde_json::json!({ "type": "message", "content": content }).to_string(),
        ))
        .await
        .unwrap();
        let broadcast = read_until_type(&mut stream, "broadcast").await;
        assert_eq!(broadcast["type"], "broadcast");
        assert_eq!(broadcast["content"], content);
        message_ids.push(
            broadcast["message_id"]
                .as_str()
                .unwrap()
                .parse::<uuid::Uuid>()
                .unwrap(),
        );
    }
    message_ids.sort_unstable();
    message_ids.dedup();
    assert_eq!(message_ids.len(), 3);

    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!(
            "{}/api/rooms/{}/messages?limit=2",
            server1, room_id
        ))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["content"], "second");
    assert_eq!(history[1]["content"], "third");

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(state1.pool())
        .await
        .unwrap();
    assert_eq!(stored, 3);

    let _ = sink.send(Message::Close(None)).await;
    drop(sink);
    drop(stream);
    server1.shutdown().await;
    state1.pool().close().await;
    drop(state1);

    let state2 = Arc::new(AppState::open(&database).await.unwrap());
    let server2 = start_server_with_state(state2.clone()).await;
    let (mut sink, mut stream) = ws_connect(&server2, &room_id, "bob", None).await;

    for expected in ["first", "second", "third"] {
        let replayed = read_json(&mut stream).await;
        assert_eq!(replayed["type"], "broadcast");
        assert_eq!(replayed["content"], expected);
        assert_eq!(replayed["sender"], "alice");
    }
    assert_eq!(read_json(&mut stream).await["type"], "system");

    let _ = sink.send(Message::Close(None)).await;
    drop(sink);
    drop(stream);
    server2.shutdown().await;
    state2.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn private_message_history_requires_room_password() {
    let server = start_server().await;
    let (room_id, _) = create_room(&server, "private-history", Some("secret")).await;
    let alice_token = session_token(&server, "alice").await;
    let (mut sink, mut stream) = ws_connect(&server, &room_id, "alice", Some("secret")).await;
    assert_eq!(read_json(&mut stream).await["type"], "system");

    sink.send(Message::Text(
        serde_json::json!({ "type": "message", "content": "private text" }).to_string(),
    ))
    .await
    .unwrap();
    assert_eq!(
        read_until_type(&mut stream, "broadcast").await["content"],
        "private text"
    );

    let url = format!("{}/api/rooms/{}/messages", server, room_id);
    let client = reqwest::Client::new();
    assert_eq!(client.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&alice_token)
            .header("x-room-password", "wrong")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    let history: Vec<serde_json::Value> = client
        .get(&url)
        .bearer_auth(&alice_token)
        .header("x-room-password", "secret")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["content"], "private text");
}

#[tokio::test]
async fn concurrent_duplicate_room_creation_returns_conflict() {
    let server = start_server().await;
    let url = format!("{}/api/rooms", server);
    let token = session_token(&server, "concurrent-owner").await;
    let request = || {
        reqwest::Client::new()
            .post(&url)
            .bearer_auth(&token)
            .json(&serde_json::json!({ "name": "same-name", "password": "" }))
            .send()
    };

    let (first, second) = tokio::join!(request(), request());
    let mut statuses = vec![
        first.unwrap().status().as_u16(),
        second.unwrap().status().as_u16(),
    ];
    statuses.sort_unstable();
    assert_eq!(statuses, vec![201, 409]);

    let rooms: Vec<serde_json::Value> = reqwest::get(&url).await.unwrap().json().await.unwrap();
    assert_eq!(rooms.len(), 1);
}

#[tokio::test]
async fn list_rooms_filter_by_name() {
    let base = start_server().await;
    create_room(&base, "alpha", None).await;
    create_room(&base, "beta", Some("pw")).await;
    create_room(&base, "gamma", None).await;

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 3);

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms?name=beta", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "beta");

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms?name=nobody", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    create_room(&base, "my room", None).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms?name=my%20room", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "my room");
}

#[tokio::test]
async fn fresh_start_creates_database_and_runs_migrations() {
    let database = temp_path("chat-rooms-fresh", "db");
    assert!(!database.exists());

    let state = Arc::new(AppState::open(&database).await.unwrap());
    assert!(database.exists());

    let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(migration_count, 9);

    let legacy_table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'app_metadata'",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(legacy_table, 0);

    let server = start_server_with_state(state.clone()).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", server))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    server.shutdown().await;
    state.pool().close().await;
    remove_sqlite_files(&database);
}
