use super::*;
use chat_room::config::{AdminConfig, AppConfig};

#[tokio::test]
async fn postgres_audit_and_room_bans_match_the_sqlite_contract() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_audit_and_room_bans_match_the_sqlite_contract").await
    else {
        return;
    };
    let (database_name, database_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["pg-audit-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(
        AppState::open_postgres(&database_url, &config)
            .await
            .unwrap(),
    );
    let server = start_server_with_state(state.clone()).await;
    let client = reqwest::Client::new();
    let admin = super::support::system_admin_token(&state, &server, "pg-audit-admin").await;

    assert_eq!(
        client
            .put(format!("{server}/api/admin/chat-lock"))
            .bearer_auth(&admin)
            .json(&serde_json::json!({ "locked": false }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let system_page: serde_json::Value = client
        .get(format!(
            "{server}/api/admin/audit-events?event_type=system.lock.update_requested"
        ))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let event_id = uuid::Uuid::parse_str(system_page["items"][0]["id"].as_str().unwrap()).unwrap();
    let pool = state.postgres_pool().unwrap();
    assert!(
        sqlx::query("UPDATE audit_events SET event_type = 'tampered' WHERE id = $1")
            .bind(event_id)
            .execute(pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM audit_events WHERE id = $1")
        .bind(event_id)
        .execute(pool)
        .await
        .is_err());

    let room_name = "pg-audit-room";
    let (room_id, _) = create_room(&server, room_name, None).await;
    let owner = session_token(&server, &format!("owner-{room_name}")).await;
    let member = session_token(&server, "pg-banned-member").await;
    let member_user = state
        .session_user(uuid::Uuid::parse_str(&member).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client
            .post(format!("{server}/api/rooms/{room_id}/invitations"))
            .bearer_auth(&owner)
            .json(&serde_json::json!({ "username": member_user.username }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let member_url = format!("{server}/api/rooms/{room_id}/members/{}", member_user.id);
    for action in ["ban", "unban"] {
        assert_eq!(
            client
                .patch(&member_url)
                .bearer_auth(&owner)
                .json(&serde_json::json!({ "action": action }))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    let room_page: serde_json::Value = client
        .get(format!(
            "{server}/api/rooms/{room_id}/audit-events?limit=20"
        ))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let serialized = serde_json::to_string(&room_page).unwrap();
    assert!(serialized.contains("room.member.ban_requested"));
    assert!(serialized.contains("room.member.unban_requested"));

    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_scratch_database(&admin_pool, &database_name).await;
}
