//! HTTP helpers and the approval / management-right half of the TG-205 scenario.

#![allow(dead_code)]

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use crate::chat_admin_support::{Account, Server};

pub async fn post(
    server: &Server,
    path: &str,
    account: &Account,
    body: Value,
) -> (StatusCode, Value) {
    server
        .call(Method::POST, path, &account.token, Some(body))
        .await
}

pub async fn links(server: &Server, base: &str, account: &Account) -> Value {
    let (status, view) = server
        .get(&format!("{base}/invite-links"), &account.token)
        .await;
    assert_eq!(status, StatusCode::OK, "{view}");
    view
}

pub async fn join(server: &Server, token: &str, account: &Account) -> (StatusCode, Value) {
    post(
        server,
        &format!("/api/invite-links/{token}/join"),
        account,
        json!({}),
    )
    .await
}

pub async fn create(server: &Server, base: &str, account: &Account, body: Value) -> Value {
    let (status, link) = post(server, &format!("{base}/invite-links"), account, body).await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    link
}

pub fn token(link: &Value) -> String {
    link["token"].as_str().unwrap().to_string()
}

/// Approval links through the existing join-request queue, the management right, and a ban.
pub async fn approval_and_rights(
    server: &Server,
    base: &str,
    owner: &Account,
    carol: &Account,
    frank: &Account,
    grace: &Account,
    erin: &Account,
) {
    let approval = create(
        server,
        base,
        owner,
        json!({ "title": "审核", "requires_approval": true }),
    )
    .await;
    let approval_id = approval["id"].as_str().unwrap();
    for account in [frank, grace] {
        let (status, queued) = join(server, &token(&approval), account).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{queued}");
        assert_eq!(queued["status"], "pending");
    }
    let (_, link) = server
        .get(&format!("{base}/invite-links"), &owner.token)
        .await;
    let listed = link["links"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == approval_id)
        .unwrap()
        .clone();
    assert_eq!(listed["pending_count"], 2);
    let (_, requests) = server
        .get(
            &format!("{base}/invite-links/{approval_id}/requests"),
            &owner.token,
        )
        .await;
    assert_eq!(requests.as_array().unwrap().len(), 2);
    // The existing approve / reject action settles them.
    for (account, action) in [(frank, "approve"), (grace, "reject")] {
        let (status, _) = server
            .call(
                Method::PATCH,
                &format!("{base}/members/{}", account.id),
                &owner.token,
                Some(json!({ "action": action })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{action}");
    }
    let (_, link) = server
        .get(&format!("{base}/invite-links"), &owner.token)
        .await;
    let listed = link["links"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == approval_id)
        .unwrap()
        .clone();
    assert_eq!(
        (
            listed["pending_count"].clone(),
            listed["usage_count"].clone()
        ),
        (json!(0), json!(1))
    );
    let (_, joined) = server
        .get(
            &format!("{base}/invite-links/{approval_id}/members"),
            &owner.token,
        )
        .await;
    assert_eq!(joined[0]["username"], "tg205-frank");

    // An administrator with members.invite (and without members.promote) manages links:
    // their own, and the primary, but not the owner's additional links.
    let (status, _) = server
        .put(
            &format!("{base}/members/{}/admin", carol.id),
            &owner.token,
            json!({ "permissions": ["members.invite"], "custom_title": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let carols = create(server, base, carol, json!({ "title": "carol" })).await;
    let view = links(server, base, carol).await;
    assert_eq!(
        (
            view["can_review"].clone(),
            view["can_manage_others"].clone()
        ),
        (json!(false), json!(false))
    );
    let (status, _) = post(
        server,
        &format!("{base}/invite-links/{approval_id}/revoke"),
        carol,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Read-time re-authorization: dismissing Carol kills the links she issued.
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("{base}/members/{}/admin", carol.id),
            &owner.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        join(server, &token(&carols), grace).await.1["error"],
        "revoked"
    );
    let (status, _) = server
        .get(&format!("{base}/invite-links"), &carol.token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A ban beats any link.
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("{base}/members/{}", erin.id),
            &owner.token,
            Some(json!({ "action": "ban" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let primary = links(server, base, owner).await["links"][0].clone();
    let (status, refused) = join(server, &token(&primary), erin).await;
    assert_eq!(
        (status, refused["error"].clone()),
        (StatusCode::FORBIDDEN, json!("banned"))
    );
    // Unknown and malformed tokens are plain 404s.
    for bogus in ["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "..%2F.."] {
        let (status, _) = join(server, bogus, erin).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
