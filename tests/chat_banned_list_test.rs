//! TG-1208: who may read a chat's banned list (`GET /api/chats/:id/members`).
//!
//! The M12 walkthrough found an administrator granted only «封禁与限制成员» (`members.ban`)
//! could ban and unban, yet «已封禁的用户» showed «读取失败»: the list read demanded
//! `members.review`. A ban-only administrator now reads exactly the banned rows — the roster
//! and the join requests stay with reviewers — and a plain member still reads nothing.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres, Account, Server};

fn statuses(list: &Value) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = list
        .as_array()
        .expect("a list")
        .iter()
        .map(|row| {
            (
                row["user_id"].as_str().unwrap_or_default().to_owned(),
                row["status"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    rows.sort();
    rows
}

async fn ban(server: &Server, chat: &str, actor: &Account, target: &Account) {
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("/api/chats/{chat}/members/{}", target.id),
            &actor.token,
            Some(json!({ "action": "ban" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "ban");
}

async fn banned_list_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let owner = server.register("tg1208-owner").await;
    let chat = server.create_group(&owner, "tg1208 banned list").await;
    let banner = server.register("tg1208-banner").await;
    let dave = server.register("tg1208-dave").await;
    let erin = server.register("tg1208-erin").await;
    for account in [&banner, &dave, &erin] {
        server.join(&chat, account).await;
    }
    let (status, _) = server
        .put(
            &format!("/api/chats/{chat}/members/{}/admin", banner.id),
            &owner.token,
            json!({ "permissions": ["members.ban"], "custom_title": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "appoint a ban-only administrator");
    ban(&server, &chat, &banner, &dave).await;

    let path = format!("/api/chats/{chat}/members");
    let (status, list) = server.get(&path, &banner.token).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a ban-only administrator reads the banned list"
    );
    assert_eq!(
        statuses(&list),
        vec![(dave.id.clone(), "banned".to_owned())],
        "only the banned rows, not the roster"
    );

    let (status, _) = server.get(&path, &erin.token).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a plain member reads nothing"
    );

    let (status, list) = server.get(&path, &owner.token).await;
    assert_eq!(status, StatusCode::OK);
    let rows = statuses(&list);
    assert!(rows.contains(&(dave.id.clone(), "banned".to_owned())));
    assert!(
        rows.contains(&(erin.id.clone(), "active".to_owned())),
        "the owner still reads the roster"
    );
}

#[tokio::test]
async fn banned_list_on_sqlite() {
    banned_list_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn banned_list_on_postgres() {
    with_postgres("banned_list_on_postgres", banned_list_scenario).await;
}
