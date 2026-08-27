use sqlx::postgres::PgPoolOptions;

pub(super) async fn connect_postgres_admin(test_name: &str) -> Option<(String, sqlx::PgPool)> {
    let configured = std::env::var("TEST_POSTGRES_ADMIN_URL").ok();
    let admin_url = configured
        .clone()
        .unwrap_or_else(|| "postgresql://postgres:postgres@localhost:52735/postgres".to_string());
    match PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
    {
        Ok(pool) => Some((admin_url, pool)),
        Err(error) if configured.is_some() => {
            panic!("{test_name}: required PostgreSQL at {admin_url} is unavailable: {error}")
        }
        Err(error) => {
            eprintln!("skipping {test_name}: could not reach PostgreSQL at {admin_url}: {error}");
            None
        }
    }
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
