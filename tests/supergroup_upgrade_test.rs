//! TG-201: the one-way `group` → `supergroup` upgrade, by each of its four triggers, keeping
//! every member and message. SQLite always, PostgreSQL when configured.

use std::sync::Arc;

use chat_room::chats::{CapabilityError, ChatCapabilityChange, ChatType, SupergroupUpgradeTrigger};
use chat_room::state::AppState;
use reqwest::StatusCode;
use serde_json::Value;
use uuid::Uuid;

mod chat_admin_support;
mod roster_seed;

use chat_admin_support::{next_frame, send_text, serve, with_postgres};
use roster_seed::{insert_account, seed_members};

async fn chat_type_of(state: &AppState, room_id: Uuid) -> ChatType {
    state.chat_type(room_id).await.unwrap().unwrap()
}

async fn member_limit_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg201-up-owner").await;
    let chat = server.create_group(&owner, "tg201-growing").await;
    let room_id = Uuid::parse_str(&chat).unwrap();
    let mut owner_socket = server.socket(&chat, &owner).await;
    for content in ["before one", "before two", "before three"] {
        send_text(&mut owner_socket, content).await;
        assert_eq!(
            next_frame(&mut owner_socket, "broadcast").await["content"],
            content
        );
    }

    // 1 owner + 199 seeded = 200: exactly the group limit, still a group.
    seed_members(&state, room_id, "tg201-up-seed-", 199).await;
    let last = insert_account(&state, "tg201-up-last").await;
    let penultimate = insert_account(&state, "tg201-up-penultimate").await;
    state
        .request_chat_membership(room_id, penultimate, true)
        .await
        .unwrap();
    assert_eq!(server.member_count(&chat, &owner.token).await, 201);
    assert_eq!(chat_type_of(&state, room_id).await, ChatType::Supergroup);
    // Joining through the domain upgraded it and told the open sockets.
    let frame = next_frame(&mut owner_socket, "chat_updated").await;
    assert_eq!(frame["chat"]["chat_type"], "supergroup");
    assert_eq!(frame["chat"]["member_count"], 201);

    // Nothing was lost: history, and every member through the paged roster.
    for content in ["before one", "before two", "before three"] {
        assert!(server.history_contains(&chat, &owner.token, content).await);
    }
    let mut seen = 0;
    let mut cursor: Option<String> = None;
    loop {
        let path = match &cursor {
            Some(cursor) => format!("/api/chats/{chat}/members/page?limit=200&cursor={cursor}"),
            None => format!("/api/chats/{chat}/members/page?limit=200"),
        };
        let (status, page) = server.get(&path, &owner.token).await;
        assert_eq!(status, StatusCode::OK);
        seen += page["items"].as_array().unwrap().len();
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
    }
    assert_eq!(seen, 201);
    // The owner is still the owner; sending still works after the upgrade.
    let (_, descriptor) = server
        .get(&format!("/api/chats/{chat}"), &owner.token)
        .await;
    assert_eq!(descriptor["chat_type"], "supergroup");
    assert_eq!(descriptor["membership_role"], "owner");
    send_text(&mut owner_socket, "after upgrade").await;
    assert_eq!(
        next_frame(&mut owner_socket, "broadcast").await["content"],
        "after upgrade"
    );

    // One way: shrinking back under the limit does not downgrade.
    state
        .request_chat_membership(room_id, last, true)
        .await
        .unwrap();
    for user in [last, penultimate] {
        state
            .delete_chat_membership(room_id, user, false)
            .await
            .unwrap();
    }
    assert_eq!(server.member_count(&chat, &owner.token).await, 200);
    assert_eq!(chat_type_of(&state, room_id).await, ChatType::Supergroup);
    let (_, list) = server.get("/api/chats", &owner.token).await;
    let listed: &Value = list
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == chat.as_str())
        .unwrap();
    assert_eq!(listed["chat_type"], "supergroup");
    assert_eq!(listed["member_count"], 200);
}

async fn capability_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg201-cap-owner").await;
    let cases = [
        (
            "tg201-cap-username",
            ChatCapabilityChange {
                username: Some(Some("tg201public".into())),
                ..Default::default()
            },
            SupergroupUpgradeTrigger::PublicUsernameSet,
        ),
        (
            "tg201-cap-forum",
            ChatCapabilityChange {
                is_forum: Some(true),
                ..Default::default()
            },
            SupergroupUpgradeTrigger::ForumEnabled,
        ),
        (
            "tg201-cap-slow",
            ChatCapabilityChange {
                slow_mode_seconds: Some(30),
                ..Default::default()
            },
            SupergroupUpgradeTrigger::SlowModeEnabled,
        ),
    ];
    for (title, change, trigger) in cases {
        let chat = server.create_group(&owner, title).await;
        let room_id = Uuid::parse_str(&chat).unwrap();
        let mut socket = server.socket(&chat, &owner).await;
        send_text(&mut socket, "kept").await;
        next_frame(&mut socket, "broadcast").await;
        let outcome = state
            .apply_chat_capabilities(room_id, change.clone())
            .await
            .unwrap();
        assert_eq!(outcome.upgraded, Some(trigger), "{title}");
        assert_eq!(outcome.chat.chat_type, ChatType::Supergroup);
        assert_eq!(chat_type_of(&state, room_id).await, ChatType::Supergroup);
        assert!(server.history_contains(&chat, &owner.token, "kept").await);
        assert_eq!(server.member_count(&chat, &owner.token).await, 1);
        // Turning the capability off again keeps the supergroup.
        let off = ChatCapabilityChange {
            username: change.username.as_ref().map(|_| None),
            is_forum: change.is_forum.map(|_| false),
            slow_mode_seconds: change.slow_mode_seconds.map(|_| 0),
        };
        let outcome = state.apply_chat_capabilities(room_id, off).await.unwrap();
        assert_eq!(outcome.upgraded, None);
        assert_eq!(outcome.chat.chat_type, ChatType::Supergroup);
        drop(socket);
    }
    // An untouched capability change on a group does not upgrade it.
    let chat = server.create_group(&owner, "tg201-cap-none").await;
    let room_id = Uuid::parse_str(&chat).unwrap();
    let outcome = state
        .apply_chat_capabilities(room_id, ChatCapabilityChange::default())
        .await
        .unwrap();
    assert_eq!(outcome.upgraded, None);
    assert_eq!(chat_type_of(&state, room_id).await, ChatType::Group);
    assert!(matches!(
        state
            .apply_chat_capabilities(Uuid::new_v4(), ChatCapabilityChange::default())
            .await,
        Err(CapabilityError::NotFound)
    ));
}

#[tokio::test]
async fn member_limit_upgrade_on_sqlite() {
    member_limit_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn member_limit_upgrade_on_postgres() {
    with_postgres("member_limit_upgrade_on_postgres", member_limit_scenario).await;
}

#[tokio::test]
async fn capability_upgrades_on_sqlite() {
    capability_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn capability_upgrades_on_postgres() {
    with_postgres("capability_upgrades_on_postgres", capability_scenario).await;
}
