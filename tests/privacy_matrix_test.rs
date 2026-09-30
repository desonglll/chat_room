//! TG-505 acceptance: every privacy dimension × every tier, asserted through the real read
//! path of that dimension, on SQLite and on PostgreSQL.
//!
//! | dimension      | read path asserted                                               |
//! |----------------|------------------------------------------------------------------|
//! | last_seen      | `auth_ok.statuses` of a WebSocket join (exact vs obscured)        |
//! | profile_photo  | `GET /api/users/:id/avatar` with the viewer's bearer (200 vs 404) |
//! | forwards       | `POST /api/messages/forward`, then the stored attribution         |
//! | group_invites  | `POST /api/chats/:id/invitations` (200 vs 403)                    |
//! | voice_messages | `AppState::voice_message_allowed` — voice messages are TG-401's   |
//!
//! Viewers: a stranger, a contact, an allow-listed stranger, a deny-listed contact, and a
//! blocked account that is ALSO allow-listed (a block beats an allow exception).

mod privacy_support;

use chat_room::{accounts::privacy::HIDDEN_FORWARD_LABEL, config::AppConfig, state::AppState};
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    start_server, start_server_on, status_of, Account, TestServer, KEYS, TIERS,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

struct Cast {
    owner: Account,
    group: Uuid,
    message: Uuid,
    avatar_url: String,
    /// (viewer, the viewer's own group used as forward target and invitation source)
    viewers: Vec<(Account, Uuid)>,
}

async fn cast(server: &TestServer) -> Cast {
    let owner = server.account("pm-owner").await;
    let stranger = server.account("pm-stranger").await;
    let contact = server.account("pm-contact").await;
    let allowed = server.account("pm-allowed").await;
    let denied = server.account("pm-denied").await;
    let blocked = server.account("pm-blocked").await;
    server.befriend(&owner, &contact).await;
    server.befriend(&owner, &denied).await;
    server.block(&owner, &blocked).await;

    let group = server.create_group(&owner, "pm-group").await;
    let avatar_url = server.upload_avatar(&owner).await;
    let mut viewers = Vec::new();
    for viewer in [stranger, contact, allowed, denied, blocked] {
        server.join(group, &viewer).await;
        let own = server
            .create_group(&viewer, &format!("{}-own", viewer.name))
            .await;
        viewers.push((viewer, own));
    }
    // Sending also connects and disconnects the owner, so a last-seen record exists.
    let message = server.send_message(group, &owner, "pm-original").await;
    Cast {
        owner,
        group,
        message,
        avatar_url,
        viewers,
    }
}

/// The expected decision for each viewer, in `cast` order.
fn expected(tier: &str) -> [bool; 5] {
    // [stranger, contact, allowed, denied, blocked+allowed]
    match tier {
        "everybody" => [true, true, true, false, false],
        "contacts" => [false, true, true, false, false],
        "nobody" => [false, false, true, false, false],
        _ => unreachable!(),
    }
}

async fn last_seen_exact(server: &TestServer, cast: &Cast, viewer: &Account) -> bool {
    let (socket, auth) = server.open_chat(cast.group, viewer).await;
    let status = status_of(&auth, cast.owner.id);
    server.close_and_settle(cast.group, viewer, socket).await;
    match status["kind"].as_str().unwrap() {
        "offline" => true,
        "recently" | "within_week" | "within_month" | "long_ago" => {
            assert!(status.get("last_seen").is_none(), "{status}");
            false
        }
        other => panic!("unexpected status {other} for an offline owner"),
    }
}

async fn photo_visible(server: &TestServer, cast: &Cast, viewer: &Account) -> bool {
    let response = server
        .client
        .get(server.url(&cast.avatar_url))
        .bearer_auth(&viewer.token)
        .send()
        .await
        .unwrap();
    match response.status() {
        StatusCode::OK => true,
        StatusCode::NOT_FOUND => false,
        other => panic!("avatar answered {other}"),
    }
}

async fn forward_attributed(
    server: &TestServer,
    cast: &Cast,
    viewer: &Account,
    target: Uuid,
) -> bool {
    let results: Vec<Value> = server
        .client
        .post(server.url("/api/messages/forward"))
        .bearer_auth(&viewer.token)
        .json(&json!({ "message_ids": [cast.message], "target_room_ids": [target] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let forwarded = results[0]["forwarded_message_id"]
        .as_str()
        .unwrap_or_else(|| panic!("forward skipped: {results:?}"))
        .to_string();
    let messages: Vec<Value> = server
        .client
        .get(server.url(&format!("/api/chats/{target}/messages")))
        .bearer_auth(&viewer.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let message = messages
        .iter()
        .find(|message| message["id"] == forwarded.as_str())
        .expect("forwarded message listed");
    let sender = message["forwarded_from"]["sender"].as_str().unwrap();
    if sender == HIDDEN_FORWARD_LABEL {
        return false;
    }
    assert_eq!(sender, cast.owner.name);
    true
}

async fn invite_allowed(server: &TestServer, cast: &Cast, viewer: &Account, own: Uuid) -> bool {
    let response = server
        .client
        .post(server.url(&format!("/api/chats/{own}/invitations")))
        .bearer_auth(&viewer.token)
        .json(&json!({ "username": cast.owner.name }))
        .send()
        .await
        .unwrap();
    match response.status() {
        StatusCode::OK => true,
        StatusCode::FORBIDDEN => false,
        other => panic!("invitation answered {other}"),
    }
}

async fn run_matrix(server: &TestServer) {
    let cast = cast(server).await;
    let allow: Vec<&Account> = vec![&cast.viewers[2].0, &cast.viewers[4].0];
    let deny: Vec<&Account> = vec![&cast.viewers[3].0];
    for tier in TIERS {
        for key in KEYS {
            let (status, rule) = server.put_rule(&cast.owner, key, tier, &allow, &deny).await;
            assert_eq!(status, StatusCode::OK, "{key}={tier}: {rule}");
            assert_eq!(rule["tier"], tier);
        }
        for ((viewer, own), want) in cast.viewers.iter().zip(expected(tier)) {
            let label = format!("tier={tier} viewer={}", viewer.name);
            assert_eq!(
                last_seen_exact(server, &cast, viewer).await,
                want,
                "last_seen {label}"
            );
            assert_eq!(
                photo_visible(server, &cast, viewer).await,
                want,
                "profile_photo {label}"
            );
            assert_eq!(
                forward_attributed(server, &cast, viewer, *own).await,
                want,
                "forwards {label}"
            );
            assert_eq!(
                invite_allowed(server, &cast, viewer, *own).await,
                want,
                "group_invites {label}"
            );
            assert_eq!(
                server
                    .state
                    .voice_message_allowed(viewer.id, cast.owner.id)
                    .await
                    .unwrap(),
                want,
                "voice_messages {label}"
            );
        }
        // The owner is never restricted by their own rule.
        for key in [
            chat_room::accounts::privacy::PrivacyKey::LastSeen,
            chat_room::accounts::privacy::PrivacyKey::ProfilePhoto,
            chat_room::accounts::privacy::PrivacyKey::Forwards,
            chat_room::accounts::privacy::PrivacyKey::GroupInvites,
            chat_room::accounts::privacy::PrivacyKey::VoiceMessages,
        ] {
            assert!(server
                .state
                .privacy_allows(cast.owner.id, key, cast.owner.id)
                .await
                .unwrap());
        }
    }
}

#[tokio::test]
async fn sqlite_every_dimension_and_tier_is_enforced_on_its_read_path() {
    let server = start_server().await;
    run_matrix(&server).await;
}

#[tokio::test]
async fn postgres_every_dimension_and_tier_is_enforced_on_its_read_path() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_every_dimension_and_tier_is_enforced_on_its_read_path").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "privacy_matrix").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    {
        let server = start_server_on(state).await;
        run_matrix(&server).await;
        server.state.postgres_pool().unwrap().close().await;
    }
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
