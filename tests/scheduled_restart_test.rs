//! TG-404: scheduled delivery is durable and exactly once, on both adapters.
//!
//! - Restart: schedule, shut the server down (pool closed, state dropped), let the delivery
//!   time pass while it is down, start a new `AppState` on the same database — the message is
//!   delivered once, promptly, and never again.
//! - Two live instances on one database race their dispatchers for the same due row — one
//!   message results.

mod migration_support;
mod poll_support;
mod scheduled_support;

use std::{path::PathBuf, sync::Arc, time::Duration};

use chat_room::{config::AppConfig, state::AppState};
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool, remove_sqlite_files,
    sqlite_scratch_path,
};
use poll_support::{create_chat, register, serve, Account};
use scheduled_support::{history_count, list, schedule_ok};
use uuid::Uuid;

enum Backend {
    Sqlite(PathBuf),
    Postgres(String),
}

impl Backend {
    async fn open(&self) -> Arc<AppState> {
        Arc::new(match self {
            Self::Sqlite(path) => AppState::open(path).await.unwrap(),
            Self::Postgres(url) => AppState::open_postgres(url, &AppConfig::default())
                .await
                .unwrap(),
        })
    }
}

async fn shut_down(state: Arc<AppState>) {
    match state.postgres_pool() {
        Some(pool) => pool.close().await,
        None => state.pool().close().await,
    }
    drop(state);
}

async fn wait_for_delivery(base: &str, account: &Account, chat: Uuid, id: Uuid) {
    for _ in 0..100 {
        if history_count(base, account, chat, id).await > 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("scheduled message {id} was not delivered");
}

async fn survives_a_restart(backend: &Backend, label: &str) {
    let first = serve(backend.open().await).await;
    let alice = register(&first.base, &format!("{label}-alice")).await;
    let chat = create_chat(&first.base, &alice, &format!("{label} restart")).await;
    let id = schedule_ok(
        &first.base,
        &alice,
        chat,
        "delivered after restart",
        2,
        false,
    )
    .await;
    let state = first.state.clone();
    drop(first);
    shut_down(state).await;

    // The delivery time passes while no server is running.
    tokio::time::sleep(Duration::from_secs(3)).await;

    let second = serve(backend.open().await).await;
    wait_for_delivery(&second.base, &alice, chat, id).await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(history_count(&second.base, &alice, chat, id).await, 1);
    let (_, pending) = list(&second.base, &alice, chat).await;
    assert!(pending.as_array().unwrap().is_empty());
    let state = second.state.clone();
    drop(second);
    shut_down(state).await;
}

async fn two_instances_deliver_once(backend: &Backend, label: &str) {
    let one = serve(backend.open().await).await;
    let two = serve(backend.open().await).await;
    let alice = register(&one.base, &format!("{label}-alice")).await;
    let chat = create_chat(&one.base, &alice, &format!("{label} race")).await;
    let mut ids = Vec::new();
    for index in 0..5 {
        ids.push(schedule_ok(&one.base, &alice, chat, &format!("race {index}"), 2, false).await);
    }
    // Both dispatchers poll the same table; the API reads go to `one` only, because the
    // chat was created there (a second instance's in-memory chat cache is not what this tests).
    for id in &ids {
        wait_for_delivery(&one.base, &alice, chat, *id).await;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    for id in &ids {
        assert_eq!(history_count(&one.base, &alice, chat, *id).await, 1);
    }
    for server in [one, two] {
        let state = server.state.clone();
        drop(server);
        shut_down(state).await;
    }
}

#[tokio::test]
async fn sqlite_scheduled_delivery_survives_a_restart() {
    let path = sqlite_scratch_path("tg404-restart");
    survives_a_restart(&Backend::Sqlite(path.clone()), "tg404sr").await;
    remove_sqlite_files(&path);
}

#[tokio::test]
async fn sqlite_two_instances_deliver_each_scheduled_message_once() {
    let path = sqlite_scratch_path("tg404-race");
    two_instances_deliver_once(&Backend::Sqlite(path.clone()), "tg404sx").await;
    remove_sqlite_files(&path);
}

#[tokio::test]
async fn postgres_scheduled_delivery_survives_a_restart() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_scheduled_delivery_survives_a_restart").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg404_restart").await;
    survives_a_restart(&Backend::Postgres(scratch.url.clone()), "tg404pr").await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_two_instances_deliver_each_scheduled_message_once() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_two_instances_deliver_each_scheduled_message_once").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg404_race").await;
    two_instances_deliver_once(&Backend::Postgres(scratch.url.clone()), "tg404px").await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
