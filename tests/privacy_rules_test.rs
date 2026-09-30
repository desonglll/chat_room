//! TG-505: the settings API contract, exception precedence, last-seen reciprocity, the live
//! presence frames.

mod privacy_support;

use chat_room::accounts::privacy::PrivacyKey;
use privacy_support::{next_json, start_server, status_of, KEYS};
use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn settings_default_to_everybody_and_round_trip_with_exceptions() {
    let server = start_server().await;
    let owner = server.account("pr-owner").await;
    let friend = server.account("pr-friend").await;
    let other = server.account("pr-other").await;

    let unauthenticated = server
        .client
        .get(server.url("/api/users/me/privacy"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let settings: Value = server
        .client
        .get(server.url("/api/users/me/privacy"))
        .bearer_auth(&owner.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rules = settings["rules"].as_array().unwrap();
    assert_eq!(
        rules
            .iter()
            .map(|rule| rule["key"].clone())
            .collect::<Vec<_>>(),
        KEYS.iter().map(|key| json!(key)).collect::<Vec<_>>()
    );
    for rule in rules {
        assert_eq!(rule["tier"], "everybody");
        assert_eq!(rule["allow_users"], json!([]));
        assert_eq!(rule["deny_users"], json!([]));
    }

    // An account in both lists is denied; the owner in a list is dropped.
    let (status, rule) = server
        .put_rule(
            &owner,
            "last_seen",
            "contacts",
            &[&friend, &other, &owner],
            &[&other],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rule["tier"], "contacts");
    assert_eq!(rule["allow_users"][0]["id"], friend.id.to_string());
    assert_eq!(rule["allow_users"].as_array().unwrap().len(), 1);
    assert_eq!(rule["deny_users"][0]["username"], "pr-other");
    assert!(!server
        .state
        .privacy_allows(owner.id, PrivacyKey::LastSeen, other.id)
        .await
        .unwrap());

    let settings: Value = server
        .client
        .get(server.url("/api/users/me/privacy"))
        .bearer_auth(&owner.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(settings["rules"][0], rule);
    assert_eq!(settings["rules"][1]["tier"], "everybody");

    // Replacing the lists replaces them, not merges.
    let (_, rule) = server
        .put_rule(&owner, "last_seen", "nobody", &[], &[])
        .await;
    assert_eq!(rule["allow_users"], json!([]));
    assert_eq!(rule["deny_users"], json!([]));
}

#[tokio::test]
async fn invalid_writes_are_refused_without_partial_effect() {
    let server = start_server().await;
    let owner = server.account("pr-invalid").await;
    let friend = server.account("pr-invalid-friend").await;
    server
        .put_rule(&owner, "forwards", "contacts", &[&friend], &[])
        .await;

    let put = |body: Value, key: &'static str| {
        let request = server
            .client
            .put(server.url(&format!("/api/users/me/privacy/{key}")))
            .bearer_auth(&owner.token)
            .json(&body);
        async move { request.send().await.unwrap().status() }
    };
    // Unknown dimension (phone numbers do not exist in this product), unknown tier.
    assert!(put(json!({ "tier": "nobody" }), "phone_number")
        .await
        .is_client_error());
    assert!(put(json!({ "tier": "friends" }), "forwards")
        .await
        .is_client_error());
    // An unknown account rolls the whole write back.
    assert_eq!(
        put(
            json!({ "tier": "nobody", "deny_user_ids": [Uuid::new_v4()] }),
            "forwards"
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    let too_many: Vec<Uuid> = (0..1001).map(|_| Uuid::new_v4()).collect();
    assert_eq!(
        put(
            json!({ "tier": "nobody", "allow_user_ids": too_many }),
            "forwards"
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    let rule = server
        .state
        .privacy_rule(owner.id, PrivacyKey::Forwards)
        .await
        .unwrap();
    assert_eq!(
        rule.tier,
        chat_room::accounts::privacy::PrivacyTier::Contacts
    );
    assert_eq!(rule.allow_users.len(), 1);
}

#[tokio::test]
async fn exceptions_beat_tiers_and_a_block_beats_everything() {
    let server = start_server().await;
    let owner = server.account("pp-owner").await;
    let contact = server.account("pp-contact").await;
    let stranger = server.account("pp-stranger").await;
    let blocked = server.account("pp-blocked").await;
    server.befriend(&owner, &contact).await;
    server.block(&owner, &blocked).await;
    let allows = |viewer: Uuid| {
        let state = server.state.clone();
        let owner = owner.id;
        async move {
            state
                .privacy_allows(owner, PrivacyKey::ProfilePhoto, viewer)
                .await
                .unwrap()
        }
    };

    // deny exception > contacts tier, for a contact.
    server
        .put_rule(&owner, "profile_photo", "contacts", &[], &[&contact])
        .await;
    assert!(!allows(contact.id).await);
    // allow exception > nobody tier, for a stranger.
    server
        .put_rule(&owner, "profile_photo", "nobody", &[&stranger], &[])
        .await;
    assert!(allows(stranger.id).await);
    assert!(!allows(contact.id).await);
    // block > allow exception > everybody tier.
    server
        .put_rule(&owner, "profile_photo", "everybody", &[&blocked], &[])
        .await;
    assert!(!allows(blocked.id).await);
    assert!(allows(stranger.id).await);
    // A dimension's exceptions never leak into another dimension.
    server
        .put_rule(&owner, "forwards", "nobody", &[&stranger], &[])
        .await;
    assert!(server
        .state
        .privacy_allows(owner.id, PrivacyKey::GroupInvites, contact.id)
        .await
        .unwrap());
    assert!(!server
        .state
        .privacy_allows(owner.id, PrivacyKey::Forwards, contact.id)
        .await
        .unwrap());
}

#[tokio::test]
async fn hiding_your_own_last_seen_hides_everyone_elses_exact_last_seen() {
    let server = start_server().await;
    let owner = server.account("rc-owner").await;
    let viewer = server.account("rc-viewer").await;
    let group = server.create_group(&owner, "rc-group").await;
    server.join(group, &owner).await;

    let (socket, auth) = server.open_chat(group, &viewer).await;
    assert_eq!(status_of(&auth, owner.id)["kind"], "offline");
    server.close_and_settle(group, &viewer, socket).await;

    server
        .put_rule(&viewer, "last_seen", "nobody", &[], &[])
        .await;
    let (socket, auth) = server.open_chat(group, &viewer).await;
    let status = status_of(&auth, owner.id);
    assert!(status.get("last_seen").is_none(), "{status}");
    assert_ne!(status["kind"], "offline");
    // The viewer's own status is always exact to themselves.
    assert_eq!(status_of(&auth, viewer.id)["kind"], "online");
    server.close_and_settle(group, &viewer, socket).await;

    // An allow exception for the owner restores reciprocity for that pair only.
    server
        .put_rule(&viewer, "last_seen", "nobody", &[&owner], &[])
        .await;
    let (socket, auth) = server.open_chat(group, &viewer).await;
    assert_eq!(status_of(&auth, owner.id)["kind"], "offline");
    server.close_and_settle(group, &viewer, socket).await;
}

/// The live leak: `user_status` frames and connected-member lists are emitted at the
/// instant of connecting, so a hidden viewer must not receive them at all.
#[tokio::test]
async fn hidden_viewers_receive_no_live_presence_of_the_owner() {
    let server = start_server().await;
    let owner = server.account("lv-owner").await;
    let hidden = server.account("lv-hidden").await;
    let admitted = server.account("lv-admitted").await;
    let group = server.create_group(&owner, "lv-group").await;
    server
        .put_rule(&owner, "last_seen", "nobody", &[&admitted], &[])
        .await;

    let (mut hidden_socket, _) = server.open_chat(group, &hidden).await;
    let (mut admitted_socket, _) = server.open_chat(group, &admitted).await;
    let (owner_socket, owner_auth) = server.open_chat(group, &owner).await;
    // Reciprocity from the owner's side: hiding their own last-seen hides the hidden
    // viewer's online state from them, while the allow-listed account stays visible.
    let connected: Vec<&Value> = owner_auth["members"].as_array().unwrap().iter().collect();
    assert!(!connected
        .iter()
        .any(|member| member["user_id"] == hidden.id.to_string()));
    assert!(connected
        .iter()
        .any(|member| member["user_id"] == admitted.id.to_string()));

    let marker = |content: &str| chat_room::models::ChatMessage::System {
        content: content.into(),
        members: None,
        participants: None,
    };

    // The owner's `user_status` frames are broadcast AFTER `auth_ok` (behind history replay)
    // and AFTER the disconnect bookkeeping, so a marker broadcast right after
    // `open_chat` / `close_and_settle` can overtake them. Wait until the admitted viewer has
    // received the owner's frame first: the chat channel is one ordered broadcast, so the
    // hidden viewer's stream then holds that same frame (if it were delivered) before the
    // marker, and the "never delivered" check below is not vacuous.
    wait_for_owner_status(&mut admitted_socket, owner.id, "online").await;
    server.state.broadcast(group, marker("lv-marker")).await;
    assert_hidden_until(&mut hidden_socket, owner.id, "lv-marker").await;

    server.close_and_settle(group, &owner, owner_socket).await;
    wait_for_owner_status(&mut admitted_socket, owner.id, "offline").await;
    server.state.broadcast(group, marker("lv-after")).await;
    assert_hidden_until(&mut hidden_socket, owner.id, "lv-after").await;
}

async fn wait_for_owner_status(socket: &mut privacy_support::Socket, owner: Uuid, kind: &str) {
    loop {
        let frame = next_json(socket).await;
        if frame["type"] == "user_status" && frame["user_id"] == owner.to_string() {
            assert_eq!(frame["status"]["kind"], kind, "{frame}");
            return;
        }
    }
}

async fn assert_hidden_until(socket: &mut privacy_support::Socket, owner: Uuid, marker: &str) {
    loop {
        let frame = next_json(socket).await;
        if frame["type"] == "system" && frame["content"] == marker {
            return;
        }
        assert!(
            !(frame["type"] == "user_status" && frame["user_id"] == owner.to_string()),
            "hidden viewer got the owner's live status: {frame}"
        );
        if let Some(members) = frame["members"].as_array() {
            assert!(
                !members
                    .iter()
                    .any(|member| member["user_id"] == owner.to_string()),
                "hidden viewer saw the owner connected in {frame}"
            );
        }
    }
}
