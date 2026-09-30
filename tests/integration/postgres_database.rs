use super::service_skip;

/// Admin connection for the PostgreSQL integration tests, via the shared visible-skip
/// contract in `tests/service_skip/mod.rs`: `TEST_POSTGRES_ADMIN_URL` unset prints one
/// greppable `SKIPPED: PostgreSQL not verified: <test_name>` marker and skips; set but
/// unreachable panics. There is no fallback URL on purpose — a fallback that cannot
/// connect disguises "misconfigured" as "not configured".
pub(super) async fn connect_postgres_admin(test_name: &str) -> Option<(String, sqlx::PgPool)> {
    service_skip::postgres_admin_pool_or_skip(test_name).await
}

pub(super) async fn create_scratch_database(
    admin_pool: &sqlx::PgPool,
    admin_url: &str,
) -> (String, String) {
    let db_name = format!("chat_room_test_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{db_name}""#))
        .execute(admin_pool)
        .await
        .unwrap();
    let base = admin_url
        .rsplit_once('/')
        .map(|(head, _)| head)
        .unwrap_or(admin_url);
    (db_name.clone(), format!("{base}/{db_name}"))
}

pub(super) async fn drop_scratch_database(admin_pool: &sqlx::PgPool, db_name: &str) {
    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(db_name)
    .execute(admin_pool)
    .await
    .ok();
    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{db_name}""#))
        .execute(admin_pool)
        .await
        .expect("drop scratch database");
}
