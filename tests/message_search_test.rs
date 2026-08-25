use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use chrono::{Duration, Utc};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use uuid::Uuid;

struct TestServer {
    base: String,
    state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base: format!("http://{address}"),
        state,
        task,
    }
}

async fn register(client: &Client, base: &str, username: &str) -> (String, Uuid) {
    let session: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        session["token"].as_str().unwrap().to_owned(),
        Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
    )
}

async fn create_room(client: &Client, base: &str, token: &str) -> Uuid {
    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": "searchable-room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

async fn insert_message(
    state: &AppState,
    room_id: Uuid,
    sender_id: Uuid,
    content: &str,
    offset_seconds: i64,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, client_message_id, created_at) \
         VALUES (?, ?, ?, 'search-owner', ?, ?, ?)",
    )
    .bind(id)
    .bind(room_id)
    .bind(sender_id)
    .bind(content)
    .bind(Uuid::new_v4())
    .bind(Utc::now() + Duration::seconds(offset_seconds))
    .execute(state.pool())
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn room_search_and_message_context_are_authorized_and_precise() {
    let server = start_server().await;
    let client = Client::new();
    let (owner_token, owner_id) = register(&client, &server.base, "search-owner").await;
    let (outsider_token, _) = register(&client, &server.base, "search-outsider").await;
    let room_id = create_room(&client, &server.base, &owner_token).await;

    let first = insert_message(
        &server.state,
        room_id,
        owner_id,
        "Project Aurora release checklist",
        0,
    )
    .await;
    let second = insert_message(&server.state, room_id, owner_id, "Aurora launch review", 1).await;
    let recalled = insert_message(&server.state, room_id, owner_id, "Aurora secret draft", 2).await;
    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(recalled)
        .execute(server.state.pool())
        .await
        .unwrap();

    let matches: Vec<serde_json::Value> = client
        .get(format!(
            "{}/api/rooms/{room_id}/messages/search?q=Aurora&limit=20",
            server.base
        ))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0]["id"], second.to_string());
    assert_eq!(matches[1]["id"], first.to_string());

    let context: Vec<serde_json::Value> = client
        .get(format!(
            "{}/api/rooms/{room_id}/messages/{}/context?limit=20",
            server.base, first
        ))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(context
        .iter()
        .any(|message| message["id"] == first.to_string()));

    assert_eq!(
        client
            .get(format!(
                "{}/api/rooms/{room_id}/messages/search?q=Aurora",
                server.base
            ))
            .bearer_auth(&outsider_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .get(format!(
                "{}/api/rooms/{room_id}/messages/search?q=%20%20",
                server.base
            ))
            .bearer_auth(&owner_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
}
