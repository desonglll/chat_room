//! TG-205: invite links end to end over HTTP, on SQLite always and on PostgreSQL when
//! configured — primary + additional links, expiry, usage limit, immediate revocation,
//! "replace primary", approval links through the existing join-request queue, per-link stats,
//! and the management right (`members.invite` held by an administrator) at write and read time.

use std::sync::Arc;
use std::time::Duration;

use chat_room::state::AppState;
use chrono::Utc;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;
mod invite_links_support;

use chat_admin_support::serve;
use chat_admin_support::with_postgres;
use invite_links_support::{approval_and_rights, create, join, links, post, token};

async fn invite_links_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg205-owner").await;
    let [dave, carol, erin, frank, grace] = [
        server.register("tg205-dave").await,
        server.register("tg205-carol").await,
        server.register("tg205-erin").await,
        server.register("tg205-frank").await,
        server.register("tg205-grace").await,
    ];
    // A password-protected chat that approves joins: a link bypasses both.
    let (status, chat) = post(
        &server,
        "/api/chats",
        &owner,
        json!({ "title": "邀请测试", "password": "secret", "join_policy": "approval" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{chat}");
    let chat_id = chat["id"].as_str().unwrap().to_string();
    let base = format!("/api/chats/{chat_id}");

    // The primary link is issued on first read.
    let view = links(&server, &base, &owner).await;
    assert_eq!(view["can_review"], true);
    let primary = view["links"][0].clone();
    assert_eq!(primary["is_primary"], true);
    assert_eq!(primary["state"], "active");
    assert_eq!(token(&primary).len(), 43);
    assert!(!token(&primary).contains(&chat_id.replace('-', "")[..8]));
    assert_eq!(links(&server, &base, &owner).await["links"], view["links"]);

    // An outsider cannot manage links, but can preview and join through one.
    let (status, _) = server
        .get(&format!("{base}/invite-links"), &dave.token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let preview_path = format!("/api/invite-links/{}", token(&primary));
    let (status, preview) = server.get(&preview_path, &dave.token).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["title"], "邀请测试");
    assert_eq!(preview["member_count"], 1);
    assert!(preview["chat_id"].is_null(), "no chat id before joining");
    let (status, joined) = join(&server, &token(&primary), &dave).await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    assert_eq!(joined["status"], "active");
    assert_eq!(joined["chat_id"], chat_id.as_str());
    // Re-using the link as a member does not consume it.
    let (status, _) = join(&server, &token(&primary), &dave).await;
    assert_eq!(status, StatusCode::OK);
    let (_, preview) = server.get(&preview_path, &dave.token).await;
    assert_eq!(preview["chat_id"], chat_id.as_str());
    assert_eq!(preview["membership_status"], "active");
    assert_eq!(server.member_count(&chat_id, &owner.token).await, 2);
    // A plain member (who may hold members.invite as a default) does not manage links.
    let (status, _) = server
        .get(&format!("{base}/invite-links"), &dave.token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Validation.
    for body in [
        json!({ "usage_limit": 0 }),
        json!({ "usage_limit": 3, "requires_approval": true }),
        json!({ "expires_at": Utc::now() - chrono::Duration::hours(1) }),
        json!({ "title": "x".repeat(33) }),
    ] {
        let (status, _) = post(&server, &format!("{base}/invite-links"), &owner, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // Usage limit.
    let limited = create(
        &server,
        &base,
        &owner,
        json!({ "title": "限一人", "usage_limit": 1 }),
    )
    .await;
    assert_eq!(
        join(&server, &token(&limited), &carol).await.0,
        StatusCode::OK
    );
    let (status, refused) = join(&server, &token(&limited), &erin).await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(refused["error"], "limit_reached");
    let (_, joined_list) = server
        .get(
            &format!(
                "{base}/invite-links/{}/members",
                limited["id"].as_str().unwrap()
            ),
            &owner.token,
        )
        .await;
    assert_eq!(joined_list.as_array().unwrap().len(), 1);
    assert_eq!(joined_list[0]["username"], "tg205-carol");

    // Expiry takes effect by the clock alone.
    let expiring = create(
        &server,
        &base,
        &owner,
        json!({ "expires_at": Utc::now() + chrono::Duration::seconds(2) }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let (status, refused) = join(&server, &token(&expiring), &erin).await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(refused["error"], "expired");

    // Revocation is effective for the very next request.
    let doomed = create(&server, &base, &owner, json!({ "title": "撤销" })).await;
    let doomed_id = doomed["id"].as_str().unwrap();
    let (status, revoked) = post(
        &server,
        &format!("{base}/invite-links/{doomed_id}/revoke"),
        &owner,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revoked["state"], "revoked");
    let (status, refused) = join(&server, &token(&doomed), &erin).await;
    assert_eq!(
        (status, refused["error"].clone()),
        (StatusCode::GONE, json!("revoked"))
    );
    let (status, _) = server
        .get(
            &format!("/api/invite-links/{}", token(&doomed)),
            &erin.token,
        )
        .await;
    assert_eq!(status, StatusCode::GONE);
    // Revoked links cannot be edited; they can be deleted. Live links cannot be deleted.
    let edit = |id: &str| format!("{base}/invite-links/{id}");
    let (status, _) = server
        .call(Method::PUT, &edit(doomed_id), &owner.token, Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = server
        .call(
            Method::DELETE,
            &edit(limited["id"].as_str().unwrap()),
            &owner.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = server
        .call(Method::DELETE, &edit(doomed_id), &owner.token, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Editing replaces the settings and keeps the token; the primary is replaced, not edited.
    let (status, edited) = server
        .call(
            Method::PUT,
            &edit(limited["id"].as_str().unwrap()),
            &owner.token,
            Some(json!({ "title": "放宽", "usage_limit": 3 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(
        (edited["title"].clone(), edited["state"].clone()),
        (json!("放宽"), json!("active"))
    );
    assert_eq!(edited["token"], limited["token"]);
    assert_eq!(
        join(&server, &token(&limited), &erin).await.0,
        StatusCode::OK
    );
    let (status, _) = server
        .call(
            Method::PUT,
            &edit(primary["id"].as_str().unwrap()),
            &owner.token,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Replace primary: the old token dies at once, a new primary takes its place.
    let (status, replaced) = post(
        &server,
        &format!("{base}/invite-links/primary"),
        &owner,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(replaced["token"], primary["token"]);
    assert_eq!(
        join(&server, &token(&primary), &frank).await.1["error"],
        "revoked"
    );
    let view = links(&server, &base, &owner).await;
    assert_eq!(view["links"][0]["id"], replaced["id"]);
    // Revoking the primary also issues a new one.
    post(
        &server,
        &format!(
            "{base}/invite-links/{}/revoke",
            replaced["id"].as_str().unwrap()
        ),
        &owner,
        json!({}),
    )
    .await;
    let view = links(&server, &base, &owner).await;
    assert_eq!(view["links"][0]["is_primary"], true);
    assert_eq!(view["links"][0]["state"], "active");
    assert_ne!(view["links"][0]["id"], replaced["id"]);

    approval_and_rights(&server, &base, &owner, &carol, &frank, &grace, &erin).await;
}

#[tokio::test]
async fn invite_links_on_sqlite() {
    invite_links_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn invite_links_on_postgres() {
    with_postgres("invite_links_on_postgres", invite_links_scenario).await;
}
