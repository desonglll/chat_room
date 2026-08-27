use std::sync::Arc;

use chat_room::{
    admin_system_admins::AdminRoleError,
    build_app,
    config::{AdminConfig, AppConfig, AuthConfig},
    state::AppState,
};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use uuid::Uuid;

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

async fn start(config: AppConfig) -> Server {
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

async fn register(client: &Client, base: &str, username: &str) -> (StatusCode, serde_json::Value) {
    let response = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.json().await.unwrap_or(serde_json::Value::Null);
    (status, body)
}

#[tokio::test]
async fn unclaimed_legacy_name_never_grants_authority_and_role_survives_rename() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["reserved-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let server = start(config).await;
    let client = Client::new();
    let (status, account) = register(&client, &server.base, "RESERVED-ADMIN").await;
    assert_eq!(status, StatusCode::CREATED);
    let token = account["token"].as_str().unwrap();
    let user_id = Uuid::parse_str(account["user"]["id"].as_str().unwrap()).unwrap();

    assert_eq!(
        client
            .get(format!("{}/api/admin/overview", server.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let marker: String = sqlx::query_scalar(
        "SELECT value FROM system_settings WHERE key = 'legacy_admin_usernames_migrated'",
    )
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(marker, "true");

    server
        .state
        .bootstrap_system_admin("reserved-admin")
        .await
        .unwrap();
    sqlx::query("UPDATE users SET username = ? WHERE id = ?")
        .bind("renamed-admin")
        .bind(user_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    assert_eq!(
        client
            .get(format!("{}/api/admin/overview", server.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert!(matches!(
        server.state.bootstrap_system_admin("renamed-admin").await,
        Err(AdminRoleError::BootstrapUnavailable)
    ));
}

#[tokio::test]
async fn legacy_configuration_imports_only_an_account_that_already_exists() {
    let state = AppState::new().await.unwrap();
    let user = state.insert_user("existing-admin", "unused").await.unwrap();
    sqlx::query(
        "UPDATE system_settings SET value = 'false' \
         WHERE key = 'legacy_admin_usernames_migrated'",
    )
    .execute(state.pool())
    .await
    .unwrap();

    assert_eq!(
        state
            .import_legacy_system_admins(&["EXISTING-ADMIN".into(), "unclaimed-admin".into()])
            .await
            .unwrap(),
        1
    );
    assert!(state.is_system_admin(user.id).await.unwrap());
    assert_eq!(
        state
            .import_legacy_system_admins(&["unclaimed-admin".into()])
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn administrators_manage_roles_without_removing_the_last_administrator() {
    let server = start(AppConfig::default()).await;
    let client = Client::new();
    let (_, first) = register(&client, &server.base, "first-admin").await;
    let (_, second) = register(&client, &server.base, "second-admin").await;
    let first_token = first["token"].as_str().unwrap();
    let second_token = second["token"].as_str().unwrap();
    let first_id = first["user"]["id"].as_str().unwrap();
    let second_id = second["user"]["id"].as_str().unwrap();
    let first_uuid = Uuid::parse_str(first_id).unwrap();
    server
        .state
        .bootstrap_system_admin("first-admin")
        .await
        .unwrap();

    let granted = client
        .put(format!(
            "{}/api/admin/system-admins/{second_id}",
            server.base
        ))
        .bearer_auth(first_token)
        .send()
        .await
        .unwrap();
    assert_eq!(granted.status(), StatusCode::OK);
    assert_eq!(
        client
            .delete(format!(
                "{}/api/admin/system-admins/{first_id}",
                server.base
            ))
            .bearer_auth(second_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(matches!(
        server
            .state
            .grant_system_admin(first_uuid, first_uuid)
            .await,
        Err(AdminRoleError::Forbidden)
    ));
    assert!(matches!(
        server
            .state
            .create_registration_invite(first_uuid, 24)
            .await,
        Err(AdminRoleError::Forbidden)
    ));
    assert_eq!(
        client
            .delete(format!(
                "{}/api/admin/system-admins/{second_id}",
                server.base
            ))
            .bearer_auth(second_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        client
            .delete(format!("{}/api/users/me", server.base))
            .bearer_auth(second_token)
            .json(&serde_json::json!({ "current_password": "test-password" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );

    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM system_admin_events ORDER BY created_at, id")
            .fetch_all(server.state.pool())
            .await
            .unwrap();
    assert_eq!(actions, ["bootstrap", "grant", "revoke"]);
}

#[tokio::test]
async fn invite_only_registration_consumes_secret_once_and_disabled_rejects_registration() {
    let invite_config = AppConfig {
        auth: AuthConfig {
            registration_mode: "invite_only".into(),
            ..AuthConfig::default()
        },
        ..AppConfig::default()
    };
    let server = start(invite_config).await;
    let client = Client::new();
    let seed = server
        .state
        .insert_user("invite-admin", "not-a-login-password")
        .await
        .unwrap();
    let admin_session = server.state.create_session(seed).await.unwrap();
    server
        .state
        .bootstrap_system_admin("invite-admin")
        .await
        .unwrap();

    let public: serde_json::Value = client
        .get(format!("{}/api/config", server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(public["registration_mode"], "invite_only");
    assert_eq!(
        register(&client, &server.base, "missing-invite").await.0,
        StatusCode::FORBIDDEN
    );

    let invite: serde_json::Value = client
        .post(format!("{}/api/admin/registration-invites", server.base))
        .bearer_auth(admin_session.token)
        .json(&serde_json::json!({ "lifetime_hours": 24 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = invite["token"].as_str().unwrap();
    let response = client
        .post(format!("{}/api/users/register", server.base))
        .json(&serde_json::json!({
            "username": "invited-user",
            "password": "test-password",
            "invite_token": token
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        client
            .post(format!("{}/api/users/register", server.base))
            .json(&serde_json::json!({
                "username": "invite-reuse",
                "password": "test-password",
                "invite_token": token
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM registration_invites")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_ne!(stored, token);

    let disabled = start(AppConfig {
        auth: AuthConfig {
            registration_mode: "disabled".into(),
            ..AuthConfig::default()
        },
        ..AppConfig::default()
    })
    .await;
    assert_eq!(
        register(&client, &disabled.base, "disabled-user").await.0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn openapi_describes_system_administrator_and_invitation_routes() {
    let server = start(AppConfig::default()).await;
    let document: serde_json::Value =
        reqwest::get(format!("{}/api-docs/openapi.json", server.base))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
    assert!(document["paths"]["/api/admin/system-admins"].is_object());
    assert!(document["paths"]["/api/admin/system-admins/{user_id}"].is_object());
    assert!(document["paths"]["/api/admin/registration-invites"].is_object());
}
