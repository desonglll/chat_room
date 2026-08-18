use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::session_token;

#[tokio::test]
async fn join_request_accepts_legacy_non_utf8_role_ids() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let owner_token = session_token(&base, "legacy-role-owner").await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({
            "name": "legacy-role-room",
            "password": "secret",
            "join_policy": "approval"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = Uuid::parse_str(room["id"].as_str().unwrap()).unwrap();

    let current_id: String =
        sqlx::query_scalar("SELECT id FROM room_roles WHERE room_id = ? AND name = 'member'")
            .bind(room_id)
            .fetch_one(state.pool())
            .await
            .unwrap();
    let mut legacy_id = room_id.as_bytes().to_vec();
    legacy_id.extend_from_slice(b":member");
    let mut transaction = state.pool().begin().await.unwrap();
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE room_role_permissions SET role_id = CAST(? AS TEXT) WHERE role_id = ?")
        .bind(&legacy_id)
        .bind(&current_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE room_roles SET id = CAST(? AS TEXT) WHERE id = ?")
        .bind(&legacy_id)
        .bind(&current_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let member_token = session_token(&base, "legacy-role-member").await;
    let response = reqwest::Client::new()
        .post(format!("{base}/api/rooms/{room_id}/join-requests"))
        .bearer_auth(member_token)
        .json(&serde_json::json!({ "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202, "{}", response.text().await.unwrap());
    task.abort();
}
