use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig, KnowledgeGraphConfig, VectorStoreConfig},
    state::AppState,
};
use chrono::Utc;
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::session_token;

struct Server {
    base: String,
    state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start() -> Server {
    let graph_token_env = "CHAT_ROOM_TEST_ADMIN_INDEX_GRAPH_TOKEN";
    std::env::set_var(graph_token_env, "test-graph-token");
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["index-admin".into()],
            ..AdminConfig::default()
        },
        vector_store: VectorStoreConfig {
            enabled: true,
            url: "http://127.0.0.1:9".into(),
            collection: "test-messages".into(),
            dimensions: 2,
            embedding_base_url: "http://127.0.0.1:9/v1".into(),
            embedding_model: "embed-test".into(),
            worker_interval_ms: 60_000,
            ..VectorStoreConfig::default()
        },
        knowledge_graph: KnowledgeGraphConfig {
            enabled: true,
            url: "http://127.0.0.1:9".into(),
            api_token_env: graph_token_env.into(),
            worker_interval_ms: 60_000,
            ..KnowledgeGraphConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

async fn create_room(base: &str, token: &str) -> Uuid {
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": "index sync room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn admin_can_resync_vector_and_graph_outboxes() {
    let server = start().await;
    let client = reqwest::Client::new();
    let admin_token = session_token(&server.base, "index-admin").await;
    let regular_token = session_token(&server.base, "index-user").await;
    let admin = server
        .state
        .session_user(Uuid::parse_str(&admin_token).unwrap())
        .await
        .unwrap()
        .unwrap();
    let room_id = create_room(&server.base, &admin_token).await;
    let active_id = Uuid::new_v4();
    let recalled_id = Uuid::new_v4();
    for (id, content, recalled_at) in [
        (active_id, "current text", None),
        (recalled_id, "recalled text", Some(Utc::now())),
    ] {
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, recalled_at, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(room_id)
        .bind(admin.id)
        .bind(&admin.username)
        .bind(content)
        .bind(recalled_at)
        .bind(Utc::now())
        .execute(server.state.pool())
        .await
        .unwrap();
    }
    sqlx::query("DELETE FROM message_index_outbox")
        .execute(server.state.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM message_graph_outbox")
        .execute(server.state.pool())
        .await
        .unwrap();

    let unauthorized = client
        .post(format!("{}/api/admin/indexes/sync", server.base))
        .bearer_auth(&regular_token)
        .json(&serde_json::json!({ "target": "vector" }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), 403);

    for target in ["vector", "graph"] {
        let response = client
            .post(format!("{}/api/admin/indexes/sync", server.base))
            .bearer_auth(&admin_token)
            .json(&serde_json::json!({ "target": target }))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(status, 200, "sync response: {body}");
        assert_eq!(body["target"], target);
        assert_eq!(body["queued_messages"], 2);
    }

    let vector_jobs: Vec<(Uuid, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT message_id, operation, attempt_count, last_error \
         FROM message_index_outbox ORDER BY message_id",
    )
    .fetch_all(server.state.pool())
    .await
    .unwrap();
    let graph_jobs: Vec<(Uuid, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT message_id, operation, attempt_count, last_error \
         FROM message_graph_outbox ORDER BY message_id",
    )
    .fetch_all(server.state.pool())
    .await
    .unwrap();
    for jobs in [vector_jobs, graph_jobs] {
        assert_eq!(jobs.len(), 2);
        assert!(jobs.contains(&(active_id, "upsert".into(), 0, None)));
        assert!(jobs.contains(&(recalled_id, "delete".into(), 0, None)));
    }
}
