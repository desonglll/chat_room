//! TG-1203: who may remove or ban whom (`PATCH /api/chats/:id/members/:user_id`).
//!
//! Two defects found in the M12 walkthrough are pinned here:
//! - banning required `members.remove`, so an administrator granted only «封禁与限制成员»
//!   (`members.ban`, Telegram's "Ban users") could restrict a member but not ban them;
//! - any administrator holding `members.remove` could remove a peer administrator appointed by
//!   the owner. Restrictions already refused administrator targets; removal and bans now agree,
//!   and only the owner removes or bans an administrator.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres, Account, Server};

async fn appoint(server: &Server, chat: &str, owner: &Account, who: &Account, rights: &[&str]) {
    let (status, _) = server
        .put(
            &format!("/api/chats/{chat}/members/{}/admin", who.id),
            &owner.token,
            json!({ "permissions": rights, "custom_title": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "appoint {rights:?}");
}

async fn act(
    server: &Server,
    chat: &str,
    actor: &Account,
    target: &Account,
    action: &str,
) -> StatusCode {
    server
        .call(
            Method::PATCH,
            &format!("/api/chats/{chat}/members/{}", target.id),
            &actor.token,
            Some(json!({ "action": action })),
        )
        .await
        .0
}

async fn governance_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let owner = server.register("tg1203-owner").await;
    let chat = server.create_group(&owner, "tg1203 governance").await;
    let mut people = Vec::new();
    for name in ["remover", "banner", "peer", "dave", "erin", "frank"] {
        let account = server.register(&format!("tg1203-{name}")).await;
        server.join(&chat, &account).await;
        people.push(account);
    }
    let [remover, banner, peer, dave, erin, frank] = &people[..] else {
        unreachable!()
    };
    appoint(
        &server,
        &chat,
        &owner,
        remover,
        &["members.remove", "members.ban"],
    )
    .await;
    appoint(&server, &chat, &owner, banner, &["members.ban"]).await;
    appoint(&server, &chat, &owner, peer, &["message.pin"]).await;

    // An administrator never removes or bans a peer administrator; the peer stays.
    assert_eq!(
        act(&server, &chat, remover, peer, "remove").await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        act(&server, &chat, remover, peer, "ban").await,
        StatusCode::CONFLICT
    );
    let (status, entry) = server
        .get(
            &format!("/api/chats/{chat}/members/{}", peer.id),
            &owner.token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(entry["role"], "admin");

    // `members.ban` alone bans and unbans; it does not remove.
    assert_eq!(
        act(&server, &chat, banner, dave, "ban").await,
        StatusCode::OK
    );
    let (status, _) = server
        .get(&format!("/api/chats/{chat}/messages"), &dave.token)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a banned member no longer reads"
    );
    assert_eq!(
        act(&server, &chat, banner, dave, "unban").await,
        StatusCode::OK
    );
    assert_eq!(
        act(&server, &chat, banner, erin, "remove").await,
        StatusCode::FORBIDDEN
    );

    // `members.remove` removes an ordinary member.
    assert_eq!(
        act(&server, &chat, remover, erin, "remove").await,
        StatusCode::OK
    );
    // A member holds neither right.
    assert_eq!(
        act(&server, &chat, frank, dave, "ban").await,
        StatusCode::FORBIDDEN
    );

    // The owner may still remove an administrator.
    assert_eq!(
        act(&server, &chat, &owner, peer, "remove").await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn member_governance_on_sqlite() {
    governance_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn member_governance_on_postgres() {
    with_postgres("member_governance_on_postgres", governance_scenario).await;
}
