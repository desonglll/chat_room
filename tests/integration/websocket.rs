use super::*;

#[tokio::test]
async fn ws_auth_wrong_password() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "secret-room", Some("correct")).await;
    let url = format!("{}/ws/{}", base.replace("http://", "ws://"), id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();
    let token = session_token(&base, "alice").await;
    let auth = serde_json::json!({ "type": "auth", "token": token, "password": "wrong" });
    sink.send(Message::Text(auth.to_string())).await.unwrap();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
        other => panic!("expected auth_fail, got {other:?}"),
    };
    let response: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(response["type"], "auth_fail");
    assert!(response["reason"].as_str().unwrap().contains("password"));
}

#[tokio::test]
async fn ws_private_join_and_chat() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "private-chat", Some("pw")).await;
    let (mut sink_a, mut stream_a) = ws_connect(&base, &id, "alice", Some("pw")).await;
    let (_sink_b, mut stream_b) = ws_connect(&base, &id, "bob", Some("pw")).await;

    let message = read_json(&mut stream_a).await;
    assert_eq!(message["type"], "system");
    assert!(message["content"].as_str().unwrap().contains("alice"));
    let message = read_json(&mut stream_a).await;
    assert_eq!(message["type"], "system");
    assert!(message["content"].as_str().unwrap().contains("bob"));
    assert_eq!(
        read_until_content(&mut stream_b, "bob").await["type"],
        "system"
    );

    sink_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "Hello Bob!" }).to_string(),
        ))
        .await
        .unwrap();
    let message = read_until_type(&mut stream_b, "broadcast").await;
    assert_eq!(message["sender"], "alice");
    assert_eq!(message["content"], "Hello Bob!");
    let message = read_until_type(&mut stream_a, "broadcast").await;
    assert_eq!(message["sender"], "alice");
    assert_eq!(message["content"], "Hello Bob!");
}

#[tokio::test]
async fn ws_client_message_id_is_idempotent_and_acknowledged() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "idempotent-chat", None).await;
    let (mut sink, mut stream) = ws_connect(&base, &id, "idempotent-user", None).await;
    let client_message_id = uuid::Uuid::new_v4();
    let payload = serde_json::json!({
        "type": "message",
        "content": "send exactly once",
        "client_message_id": client_message_id,
    })
    .to_string();

    sink.send(Message::Text(payload.clone())).await.unwrap();
    let first = read_until_type(&mut stream, "broadcast").await;
    assert_eq!(first["client_message_id"], client_message_id.to_string());

    sink.send(Message::Text(payload)).await.unwrap();
    let retry = read_until_type(&mut stream, "broadcast").await;
    assert_eq!(retry["message_id"], first["message_id"]);

    let token = session_token(&base, "idempotent-user").await;
    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{base}/api/rooms/{id}/messages"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0]["client_message_id"],
        client_message_id.to_string()
    );
}

#[tokio::test]
async fn ws_leave_notifies_others() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "room", Some("pw")).await;
    let (_sink_a, mut stream_a) = ws_connect(&base, &id, "alice", Some("pw")).await;
    let (sink_b, stream_b) = ws_connect(&base, &id, "bob", Some("pw")).await;
    assert!(read_json(&mut stream_a).await["content"]
        .as_str()
        .unwrap()
        .contains("alice"));
    assert!(read_json(&mut stream_a).await["content"]
        .as_str()
        .unwrap()
        .contains("bob"));
    drop(sink_b);
    drop(stream_b);
    let message = read_until_type(&mut stream_a, "presence").await;
    assert_eq!(message["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn ws_nonexistent_room() {
    let base = start_server().await;
    let url = format!(
        "{}/ws/00000000-0000-0000-0000-000000000000",
        base.replace("http://", "ws://")
    );
    let (ws, _) = connect_async(&url).await.unwrap();
    let (_, mut stream) = ws.split();
    let raw = match stream.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
        other => panic!("expected auth_fail, got {other:?}"),
    };
    let response: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(response["type"], "auth_fail");
}

#[tokio::test]
async fn ws_public_join_no_password() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "public-lounge", None).await;
    assert!(!has_password);
    let (_sink, _stream) = ws_connect(&base, &id, "guest", None).await;
}

#[tokio::test]
async fn ws_public_join_with_auth_also_works() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "open", None).await;
    let (_sink, _stream) = ws_connect(&base, &id, "alice", Some("anything")).await;
}

#[tokio::test]
async fn ws_public_join_rejects_if_private() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "vip-room", Some("secret")).await;
    let url = format!("{}/ws/{}", base.replace("http://", "ws://"), id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();
    let token = session_token(&base, "alice").await;
    let join = serde_json::json!({ "type": "join", "token": token });
    sink.send(Message::Text(join.to_string())).await.unwrap();
    let raw = match stream.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
        other => panic!("expected auth_fail, got {other:?}"),
    };
    let response: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(response["type"], "auth_fail");
    assert!(response["reason"].as_str().unwrap().contains("password"));
}
