use super::*;
use chat_room::config::AppConfig;

#[tokio::test]
async fn postgres_is_probed_by_readiness_and_metrics() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_is_probed_by_readiness_and_metrics").await
    else {
        return;
    };
    let (database_name, database_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&database_url, &AppConfig::default())
            .await
            .unwrap(),
    );
    let server = start_server_with_state(state.clone()).await;
    let client = reqwest::Client::new();

    let ready = client
        .get(format!("{server}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(ready.status(), 200);
    let ready: serde_json::Value = ready.json().await.unwrap();
    assert_eq!(ready["dependencies"][0]["id"], "database");
    assert_eq!(ready["dependencies"][0]["status"], "healthy");
    let metrics = client
        .get(format!("{server}/metrics"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        metrics.contains("chat_room_dependency_up{dependency=\"database\",required=\"true\"} 1")
    );

    server.shutdown().await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_scratch_database(&admin_pool, &database_name).await;
}
