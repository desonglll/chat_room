//! TG-304: run one server flow on a SQLite state and on a PostgreSQL scratch state, so the
//! entity and emoji-status SQL is exercised on both adapters.
#![allow(dead_code)]

use std::future::Future;
use std::sync::Arc;

use chat_room::{config::AppConfig, state::AppState};

use crate::migration_support::{
    create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool,
};
use crate::sticker_support::http::{self, Server};

pub async fn on_sqlite<F, Fut>(flow: F)
where
    F: FnOnce(Server) -> Fut,
    Fut: Future<Output = ()>,
{
    flow(http::start(AppConfig::default()).await).await;
}

/// Skips visibly when `TEST_POSTGRES_ADMIN_URL` is unset; panics when set but unreachable.
pub async fn on_postgres<F, Fut>(test_name: &str, flow: F)
where
    F: FnOnce(Server) -> Fut,
    Fut: Future<Output = ()>,
{
    let Some((admin_url, admin_pool)) = postgres_admin_pool(test_name).await else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "custom_emoji").await;
    let mut config = AppConfig::default();
    config.attachments.directory =
        std::env::temp_dir().join(format!("tg304-attachments-{}", uuid::Uuid::new_v4()));
    let state = Arc::new(
        AppState::open_postgres(&scratch.url, &config)
            .await
            .unwrap(),
    );
    let pool = state.postgres_pool().unwrap().clone();
    flow(http::start_with_state(state).await).await;
    pool.close().await;
    let _ = std::fs::remove_dir_all(&config.attachments.directory);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
