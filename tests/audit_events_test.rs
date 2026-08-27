mod support;

use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use tokio::net::TcpListener;

async fn start_server() -> (String, Arc<AppState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state, task)
}

#[tokio::test]
async fn system_audit_is_authorized_minimal_and_append_only() {
    let (base, state, task) = start_server().await;
    let client = reqwest::Client::new();
    let admin_token = support::system_admin_token(&state, &base, "audit-admin").await;
    let member_token = support::session_token(&base, "audit-member").await;

    let update = client
        .put(format!("{base}/api/admin/chat-lock"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({ "locked": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);

    let response = client
        .get(format!(
            "{base}/api/admin/audit-events?event_type=system.lock.update_requested"
        ))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let page = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let event = &page["items"][0];
    assert_eq!(event["scope"], "system");
    assert_eq!(event["actor_username"], "audit-admin");
    assert_eq!(event["event_type"], "system.lock.update_requested");
    assert_eq!(event["target_type"], "system");
    assert_eq!(event["details"]["locked"], "true");
    assert!(event.get("token").is_none());
    assert!(event.get("password").is_none());
    assert!(event.get("message").is_none());
    assert!(page["next_cursor"].is_null());

    let forbidden = client
        .get(format!("{base}/api/admin/audit-events"))
        .bearer_auth(member_token)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);

    let event_id = uuid::Uuid::parse_str(event["id"].as_str().unwrap()).unwrap();
    assert!(
        sqlx::query("UPDATE audit_events SET event_type = 'tampered' WHERE id = ?")
            .bind(event_id)
            .execute(state.pool())
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM audit_events WHERE id = ?")
        .bind(event_id)
        .execute(state.pool())
        .await
        .is_err());

    task.abort();
}

async fn account(client: &reqwest::Client, base: &str, username: &str) -> (String, String) {
    let response = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": username,
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    (
        response["token"].as_str().unwrap().to_owned(),
        response["user"]["id"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn room_governance_is_audited_filtered_and_manager_only() {
    let (base, _state, task) = start_server().await;
    let client = reqwest::Client::new();
    let (owner_token, _) = account(&client, &base, "room-audit-owner").await;
    let (member_token, member_id) = account(&client, &base, "room-audit-member").await;
    let room = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({
            "name": "audited-room",
            "password": "",
            "join_policy": "approval"
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    let join_url = format!("{base}/api/rooms/{room_id}/join-requests");
    let member_url = format!("{base}/api/rooms/{room_id}/members/{member_id}");

    assert_eq!(
        client
            .post(format!("{base}/api/rooms/{room_id}/invitations"))
            .bearer_auth(&owner_token)
            .json(&serde_json::json!({ "username": "room-audit-member" }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .post(&join_url)
            .bearer_auth(&member_token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{base}/api/rooms/{room_id}/audit-events"))
            .bearer_auth(&member_token)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for payload in [
        serde_json::json!({ "action": "set_role", "role": "admin" }),
        serde_json::json!({ "action": "ban" }),
    ] {
        assert_eq!(
            client
                .patch(&member_url)
                .bearer_auth(&owner_token)
                .json(&payload)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    let members = client
        .get(format!("{base}/api/rooms/{room_id}/members"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert!(members
        .iter()
        .any(|member| member["user_id"] == member_id && member["status"] == "banned"));
    assert_eq!(
        client
            .post(&join_url)
            .bearer_auth(&member_token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let unbanned = client
        .patch(&member_url)
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "action": "unban" }))
        .send()
        .await
        .unwrap();
    assert_eq!(unbanned.status(), 200);
    assert_eq!(unbanned.json::<serde_json::Value>().await.unwrap()["status"], "banned");
    assert_eq!(
        client
            .post(&join_url)
            .bearer_auth(&member_token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        202
    );
    for action in ["reject", "approve", "remove"] {
        if action == "approve" {
            assert_eq!(
                client
                    .post(&join_url)
                    .bearer_auth(&member_token)
                    .json(&serde_json::json!({}))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                202
            );
        }
        assert_eq!(
            client
                .patch(&member_url)
                .bearer_auth(&owner_token)
                .json(&serde_json::json!({ "action": action }))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }

    let audit_url = format!("{base}/api/rooms/{room_id}/audit-events");
    let page = client
        .get(format!("{audit_url}?actor=room-audit-owner&limit=2"))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    let cursor = page["next_cursor"].as_str().unwrap();
    let next = client
        .get(&audit_url)
        .query(&[("cursor", cursor), ("limit", "2")])
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_ne!(page["items"][0]["id"], next["items"][0]["id"]);
    let ban = client
        .get(&audit_url)
        .query(&[("event_type", "room.member.ban_requested")])
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(ban["items"].as_array().unwrap().len(), 1);
    assert_eq!(ban["items"][0]["target_id"], member_id);

    task.abort();
}
