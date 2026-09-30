use chat_room::{
    admin_system_admins::AdminRoleError,
    config::{AppConfig, AuthConfig},
    state::AppState,
};
use uuid::Uuid;

mod service_skip;

#[tokio::test]
async fn postgres_enforces_bootstrap_role_and_invitation_invariants() {
    let Some((admin_url, admin_pool)) = service_skip::postgres_admin_pool_or_skip(
        "postgres_enforces_bootstrap_role_and_invitation_invariants",
    )
    .await
    else {
        return;
    };
    let database_name = format!("chat_room_admin_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database_url = format!(
        "{}/{}",
        admin_url.rsplit_once('/').unwrap().0,
        database_name
    );
    let config = AppConfig {
        auth: AuthConfig {
            registration_mode: "invite_only".into(),
            ..AuthConfig::default()
        },
        ..AppConfig::default()
    };
    let state = AppState::open_postgres(&database_url, &config)
        .await
        .unwrap();
    let first = state.insert_user("pg-first-admin", "unused").await.unwrap();
    let second = state
        .insert_user("pg-second-admin", "unused")
        .await
        .unwrap();

    state
        .bootstrap_system_admin("PG-FIRST-ADMIN")
        .await
        .unwrap();
    state.grant_system_admin(first.id, second.id).await.unwrap();
    state
        .revoke_system_admin(second.id, first.id)
        .await
        .unwrap();
    assert!(matches!(
        state.revoke_system_admin(second.id, second.id).await,
        Err(AdminRoleError::LastAdministrator)
    ));

    let (invite, _) = state
        .create_registration_invite(second.id, 24)
        .await
        .unwrap();
    state
        .register_user("pg-invited", "unused", Some(&invite))
        .await
        .unwrap();
    assert!(matches!(
        state
            .register_user("pg-invite-reuse", "unused", Some(&invite))
            .await,
        Err(chat_room::registration::RegistrationError::InvitationRequired)
    ));
    assert_eq!(state.list_system_admins().await.unwrap().len(), 1);

    drop(state);
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&database_name)
    .execute(&admin_pool)
    .await
    .ok();
    sqlx::query(&format!(r#"DROP DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
}
