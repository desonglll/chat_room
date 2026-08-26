use std::sync::{Arc, Mutex};

use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::any,
    Json, Router,
};
use chat_room::{
    build_app,
    config::{AppConfig, KnowledgeGraphConfig},
    state::AppState,
};
use chrono::Utc;
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::session_token;

#[derive(Clone, Default)]
struct FakeGraphState {
    message_id: Arc<Mutex<Option<Uuid>>>,
    authorizations: Arc<Mutex<Vec<String>>>,
}

#[tokio::test]
async fn room_graph_reauthorizes_all_sources_and_current_membership() {
    let fake_state = FakeGraphState::default();
    let fake_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fake_url = format!("http://{}", fake_listener.local_addr().unwrap());
    let fake_task = tokio::spawn({
        let state = fake_state.clone();
        async move {
            axum::serve(
                fake_listener,
                Router::new().fallback(any(fake_graph)).with_state(state),
            )
            .await
            .unwrap()
        }
    });

    let token_env = "CHAT_ROOM_TEST_KNOWLEDGE_GRAPH_TOKEN";
    std::env::set_var(token_env, "internal-graph-token-for-tests");
    let config = AppConfig {
        knowledge_graph: KnowledgeGraphConfig {
            enabled: true,
            url: fake_url,
            api_token_env: token_env.into(),
            worker_interval_ms: 10_000,
            ..KnowledgeGraphConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app_task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let owner_token = session_token(&base, "graph-api-owner").await;
    let owner = state
        .session_user(Uuid::parse_str(&owner_token).unwrap())
        .await
        .unwrap()
        .unwrap();
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "name": "graph API room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = Uuid::parse_str(room["id"].as_str().unwrap()).unwrap();
    let message_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages \
         (id, room_id, sender_id, sender, content, client_message_id, created_at) \
         VALUES (?, ?, ?, ?, 'Launch is Friday', ?, ?)",
    )
    .bind(message_id)
    .bind(room_id)
    .bind(owner.id)
    .bind(&owner.username)
    .bind(Uuid::new_v4())
    .bind(Utc::now())
    .execute(state.pool())
    .await
    .unwrap();
    *fake_state.message_id.lock().unwrap() = Some(message_id);

    let response = reqwest::Client::new()
        .get(format!("{base}/api/rooms/{room_id}/knowledge-graph"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let graph: serde_json::Value = response.json().await.unwrap();
    assert_eq!(graph["facts"].as_array().unwrap().len(), 1);
    assert_eq!(graph["facts"][0]["fact"], "Authorized fact");
    assert_eq!(graph["nodes"].as_array().unwrap().len(), 2);
    assert!(graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["summary"] == ""));
    assert!(fake_state
        .authorizations
        .lock()
        .unwrap()
        .iter()
        .any(|value| value == "Bearer internal-graph-token-for-tests"));

    let outsider_token = session_token(&base, "graph-api-outsider").await;
    let outsider = reqwest::Client::new()
        .get(format!("{base}/api/rooms/{room_id}/knowledge-graph"))
        .bearer_auth(outsider_token)
        .send()
        .await
        .unwrap();
    assert_eq!(outsider.status(), StatusCode::FORBIDDEN);
    app_task.abort();
    fake_task.abort();
}

async fn fake_graph(State(state): State<FakeGraphState>, request: Request) -> impl IntoResponse {
    if let Some(value) = request
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
    {
        state.authorizations.lock().unwrap().push(value.into());
    }
    if request.method() == Method::GET && request.uri().path().ends_with("/graph") {
        let source = state.message_id.lock().unwrap().unwrap();
        let room_id = request
            .uri()
            .path()
            .split('/')
            .nth_back(1)
            .and_then(|value| Uuid::parse_str(value).ok())
            .unwrap();
        return Json(snapshot(room_id, source)).into_response();
    }
    if request.method() == Method::GET && request.uri().path() == "/healthz" {
        return Json(serde_json::json!({ "status": "ready" })).into_response();
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "indexed" })),
    )
        .into_response()
}

fn snapshot(room_id: Uuid, authorized_message: Uuid) -> serde_json::Value {
    let [a, b, c, d] = std::array::from_fn(|_| Uuid::new_v4());
    let nodes = [a, b, c, d].map(|id| {
        serde_json::json!({
            "id": id, "name": id.to_string(), "summary": "unscoped derived summary", "labels": []
        })
    });
    let fact = |text: &str, source_node: Uuid, target_node: Uuid, message_id: Uuid| {
        serde_json::json!({
            "id": Uuid::new_v4(), "name": "relates_to", "fact": text,
            "source_node_id": source_node, "target_node_id": target_node,
            "episode_ids": [message_id], "valid_at": null, "invalid_at": null,
            "created_at": Utc::now(), "expired_at": null
        })
    };
    serde_json::json!({
        "room_id": room_id,
        "nodes": nodes,
        "facts": [
            fact("Authorized fact", a, b, authorized_message),
            fact("Unauthorized fact", c, d, Uuid::new_v4())
        ],
        "truncated": false
    })
}
