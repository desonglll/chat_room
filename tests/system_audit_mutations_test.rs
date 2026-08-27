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
async fn critical_system_mutations_are_audited_and_fail_closed() {
    let (base, state, task) = start_server().await;
    let client = reqwest::Client::new();
    let admin_token = support::system_admin_token(&state, &base, "mutation-admin").await;
    let target_token = support::session_token(&base, "mutation-target").await;
    let target_id = client
        .get(format!("{base}/api/users/me"))
        .bearer_auth(&target_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        client
            .put(format!("{base}/api/admin/system-admins/{target_id}"))
            .bearer_auth(&admin_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .delete(format!("{base}/api/admin/system-admins/{target_id}"))
            .bearer_auth(&admin_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .post(format!("{base}/api/admin/registration-invites"))
            .bearer_auth(&admin_token)
            .json(&serde_json::json!({ "lifetime_hours": 12 }))
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    let room = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({ "name": "audit-lock-room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    assert_eq!(
        client
            .put(format!("{base}/api/admin/room-locks/{room_id}"))
            .bearer_auth(&admin_token)
            .json(&serde_json::json!({ "locked": true }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    let page = client
        .get(format!("{base}/api/admin/audit-events?limit=100"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let event_types = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    for expected in [
        "system_admin.grant_requested",
        "system_admin.revoke_requested",
        "registration_invite.create_requested",
        "room.lock.update_requested",
    ] {
        assert!(event_types.contains(expected), "missing {expected}");
    }

    sqlx::query("DROP TABLE audit_events")
        .execute(state.pool())
        .await
        .unwrap();
    let rejected = client
        .put(format!("{base}/api/admin/chat-lock"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({ "locked": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 500);
    assert!(!state.chat_rooms_locked().await.unwrap());

    task.abort();
}
