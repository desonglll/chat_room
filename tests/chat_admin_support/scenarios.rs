//! Scenario pieces of `chat_admin_test.rs`, split out to keep that file under the size gate.

use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::{Account, Server};

pub fn keys(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap())
        .collect()
}

pub async fn my_permissions(server: &Server, chat: &str, account: &Account) -> Vec<String> {
    let (status, view) = server
        .get(&format!("/api/chats/{chat}/permissions"), &account.token)
        .await;
    assert_eq!(status, StatusCode::OK);
    keys(&view["my_permissions"])
        .into_iter()
        .map(String::from)
        .collect()
}

pub async fn member_count_is_maintained(
    server: &Server,
    owner: &Account,
) -> (String, Vec<Account>) {
    let chat = server.create_group(owner, "tg201-counted").await;
    assert_eq!(server.member_count(&chat, &owner.token).await, 1);
    let mut members = Vec::new();
    for name in ["tg201-bob", "tg201-carol", "tg201-dave", "tg201-erin"] {
        let account = server.register(name).await;
        server.join(&chat, &account).await;
        members.push(account);
    }
    assert_eq!(server.member_count(&chat, &owner.token).await, 5, "joins");
    // The chat list carries the same projection.
    let (_, listed) = server.get("/api/chats", &owner.token).await;
    let listed = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == chat.as_str())
        .unwrap()
        .clone();
    assert_eq!(listed["member_count"], 5);

    let erin = members.pop().unwrap();
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/chats/{chat}/members/me"),
            &erin.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(server.member_count(&chat, &owner.token).await, 4, "leave");

    let dave = members.pop().unwrap();
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("/api/chats/{chat}/members/{}", dave.id),
            &owner.token,
            Some(json!({ "action": "remove" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server.member_count(&chat, &owner.token).await, 3, "kick");
    server.join(&chat, &dave).await;
    assert_eq!(server.member_count(&chat, &owner.token).await, 4, "rejoin");
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("/api/chats/{chat}/members/{}", dave.id),
            &owner.token,
            Some(json!({ "action": "ban" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server.member_count(&chat, &owner.token).await, 3, "ban");
    (chat, members)
}

pub async fn default_permissions_scenario(
    server: &Server,
    chat: &str,
    owner: &Account,
    bob: &Account,
    carol: &Account,
) {
    let base = format!("/api/chats/{chat}");
    let rename = |title: &str| json!({ "title": title });
    // chat.info is off by default for members.
    let (status, _) = server
        .call(
            Method::PATCH,
            &base,
            &carol.token,
            Some(rename("tg201-by-carol")),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, view) = server
        .put(
            &format!("{base}/default-permissions"),
            &owner.token,
            json!({ "permissions": ["message.send", "chat.info"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        keys(&view["default_permissions"]),
        ["message.send", "chat.info"]
    );
    let (status, _) = server
        .call(
            Method::PATCH,
            &base,
            &carol.token,
            Some(rename("tg201-by-carol")),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "defaults apply at write time");
    // A per-member restriction still overrides the group default (step 4 after step 3).
    let (status, _) = server
        .put(
            &format!("{base}/members/{}/restrictions", bob.id),
            &owner.token,
            json!({ "denied_permissions": ["chat.info"], "until": null }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .call(
            Method::PATCH,
            &base,
            &bob.token,
            Some(rename("tg201-by-bob")),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Media was switched off with the defaults; the baseline (own messages) never is.
    let rights = my_permissions(server, chat, carol).await;
    assert!(!rights.contains(&"message.send_media".to_string()));
    assert!(rights.contains(&"message.edit_own".to_string()));
    // Unknown keys and admin-only keys are rejected.
    let (status, _) = server
        .put(
            &format!("{base}/default-permissions"),
            &owner.token,
            json!({ "permissions": ["members.ban"] }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Read time: a non-member reads neither the permissions nor the roster.
    let stranger = server.register("tg201-stranger").await;
    for path in [
        format!("{base}/permissions"),
        format!("{base}/members/page"),
        format!("{base}/members/{}", bob.id),
    ] {
        let (status, _) = server.get(&path, &stranger.token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
}
