//! The chat authorization decision: the order it evaluates in, the `chat_type` layer TG-005
//! added on top, and the rule from `AGENTS.md` and `docs/tg/agent-protocol.md` §6 that read
//! paths are authorized as well as write paths.

use std::sync::Arc;

use chat_room::{
    build_app,
    chats::{ChatAuthorization, ChatType},
    state::AppState,
};
use tokio::{net::TcpListener, task::JoinHandle};
use uuid::Uuid;

mod support;
use support::{session_token, system_admin_token};

/// Read endpoints that must refuse a signed-in non-member. They are listed as suffixes so each
/// one is exercised through the canonical path and through the deprecated alias.
const AUTHORIZED_READ_SUFFIXES: [&str; 6] = [
    "/{id}/messages",
    "/{id}/messages/search?q=anything",
    "/{id}/files",
    "/{id}/pins",
    "/{id}/tasks",
    "/{id}/members",
];

async fn start_server() -> (String, Arc<AppState>, JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state, task)
}

async fn create_chat(base: &str, owner_token: &str, title: &str) -> Uuid {
    create_chat_with_policy(base, owner_token, title, "approval").await
}

async fn create_chat_with_policy(
    base: &str,
    owner_token: &str,
    title: &str,
    join_policy: &str,
) -> Uuid {
    let created: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "title": title, "join_policy": join_policy }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(created["id"].as_str().unwrap()).unwrap()
}

async fn status(base: &str, path: &str, token: &str) -> reqwest::StatusCode {
    reqwest::Client::new()
        .get(format!("{base}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn every_chat_read_path_refuses_a_non_member_on_both_the_contract_and_the_alias() {
    let (base, _state, _task) = start_server().await;
    let owner = session_token(&base, "read-auth-owner").await;
    let outsider = session_token(&base, "read-auth-outsider").await;
    let chat_id = create_chat(&base, &owner, "read-auth-chat").await;

    for suffix in AUTHORIZED_READ_SUFFIXES {
        let suffix = suffix.replace("{id}", &chat_id.to_string());
        for prefix in ["/api/chats"] {
            let path = format!("{prefix}{suffix}");
            let refused = status(&base, &path, &outsider).await;
            assert_eq!(
                refused,
                reqwest::StatusCode::FORBIDDEN,
                "GET {path} let a non-member read"
            );
        }
    }

    // The same reads succeed for the owner, so the assertions above are about authorization and
    // not about a broken route.
    for suffix in AUTHORIZED_READ_SUFFIXES {
        let suffix = suffix.replace("{id}", &chat_id.to_string());
        let allowed = status(&base, &format!("/api/chats{suffix}"), &owner).await;
        assert!(
            allowed.is_success(),
            "GET /api/chats{suffix} refused the owner with {allowed}"
        );
    }
}

#[tokio::test]
async fn revoking_membership_closes_the_read_paths_not_only_the_write_paths() {
    let (base, state, _task) = start_server().await;
    let owner = session_token(&base, "revoke-auth-owner").await;
    let member = session_token(&base, "revoke-auth-member").await;
    let chat_id = create_chat_with_policy(&base, &owner, "revoke-auth-chat", "open").await;
    let member_user = state
        .session_user(Uuid::parse_str(&member).unwrap())
        .await
        .unwrap()
        .unwrap();

    // An open chat activates the joiner immediately, which is the state the revocation below
    // has to undo.
    let joined = reqwest::Client::new()
        .post(format!("{base}/api/chats/{chat_id}/join-requests"))
        .bearer_auth(&member)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert!(joined.status().is_success(), "joining an open chat failed");
    assert!(
        status(&base, &format!("/api/chats/{chat_id}/messages"), &member)
            .await
            .is_success(),
        "the new member cannot read the history it just joined"
    );

    assert_eq!(
        reqwest::Client::new()
            .patch(format!(
                "{base}/api/chats/{chat_id}/members/{}",
                member_user.id
            ))
            .bearer_auth(&owner)
            .json(&serde_json::json!({ "action": "remove" }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        status(&base, &format!("/api/chats/{chat_id}/messages"), &member).await,
        reqwest::StatusCode::FORBIDDEN,
        "history stayed readable after the membership was revoked"
    );
}

/// Forces a chat's type in the database, which is how M2's type changes will arrive before
/// there is an endpoint for them.
async fn set_chat_type(state: &AppState, chat_id: Uuid, chat_type: ChatType) {
    sqlx::query("UPDATE chats SET chat_type = $1 WHERE id = $2")
        .bind(chat_type.as_str())
        .bind(chat_id)
        .execute(state.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn the_chat_type_layer_turns_an_allow_into_a_deny_and_never_the_reverse() {
    let (base, state, _task) = start_server().await;
    let owner = session_token(&base, "type-auth-owner").await;
    let chat_id = create_chat(&base, &owner, "type-auth-chat").await;
    let owner_user = state
        .session_user(Uuid::parse_str(&owner).unwrap())
        .await
        .unwrap()
        .unwrap();

    // A group owner holds every role permission the registry ships with.
    for key in ["message.send", "members.invite", "room.settings"] {
        assert_eq!(
            state
                .authorize_chat_action(chat_id, owner_user.id, key)
                .await
                .unwrap(),
            ChatAuthorization::Creator,
            "a group creator should hold {key}"
        );
    }
    // ...but not a permission the registry does not grant a group.
    assert_eq!(
        state
            .authorize_chat_action(chat_id, owner_user.id, "message.post")
            .await
            .unwrap(),
        ChatAuthorization::ForbiddenByChatType(ChatType::Group),
        "a group is not a channel"
    );

    set_chat_type(&state, chat_id, ChatType::Private).await;
    for key in [
        "members.invite",
        "members.roles",
        "room.settings",
        "room.delete",
    ] {
        assert_eq!(
            state
                .authorize_chat_action(chat_id, owner_user.id, key)
                .await
                .unwrap(),
            ChatAuthorization::ForbiddenByChatType(ChatType::Private),
            "a one-to-one chat has no {key} to exercise"
        );
    }
    assert_eq!(
        state
            .authorize_chat_action(chat_id, owner_user.id, "message.send")
            .await
            .unwrap(),
        ChatAuthorization::Creator,
        "a one-to-one chat is still a place to talk"
    );

    set_chat_type(&state, chat_id, ChatType::Channel).await;
    // TG-202: in a channel sending *is* posting — `message.send` is decided as `message.post`
    // (`ChatType::effective_permission`), which the creator holds.
    assert_eq!(
        state
            .authorize_chat_action(chat_id, owner_user.id, "message.send")
            .await
            .unwrap(),
        ChatAuthorization::Creator,
        "a channel creator posts"
    );
    assert_eq!(
        state
            .authorize_chat_action(chat_id, owner_user.id, "chat.topics")
            .await
            .unwrap(),
        ChatAuthorization::ForbiddenByChatType(ChatType::Channel),
        "a channel is not a forum"
    );

    set_chat_type(&state, chat_id, ChatType::Supergroup).await;
    for key in ["message.send", "message.post", "chat.topics"] {
        assert_eq!(
            state
                .authorize_chat_action(chat_id, owner_user.id, key)
                .await
                .unwrap(),
            ChatAuthorization::Creator,
            "a supergroup adds no intrinsic denial for {key}"
        );
    }
}

#[tokio::test]
async fn a_system_administrator_administers_a_chat_but_cannot_reach_its_messages() {
    let (base, state, _task) = start_server().await;
    let owner = session_token(&base, "sysadmin-auth-owner").await;
    let admin = system_admin_token(&state, &base, "sysadmin-auth-admin").await;
    let chat_id = create_chat(&base, &owner, "sysadmin-auth-chat").await;
    let admin_user = state
        .session_user(Uuid::parse_str(&admin).unwrap())
        .await
        .unwrap()
        .unwrap();

    for key in ["room.settings", "room.delete", "members.remove"] {
        assert_eq!(
            state
                .authorize_chat_action(chat_id, admin_user.id, key)
                .await
                .unwrap(),
            ChatAuthorization::SystemAdministrator,
            "a deployment-wide administrator should hold {key}"
        );
    }
    // CONTEXT.md scopes a System Administrator to deployment-wide *operations*, and AGENTS.md
    // keeps the chat as the knowledge-isolation boundary: administering a chat is not the same
    // as being able to read or write inside it.
    for key in ["message.send", "message.pin", "message.edit_own"] {
        assert_eq!(
            state
                .authorize_chat_action(chat_id, admin_user.id, key)
                .await
                .unwrap(),
            ChatAuthorization::NoRolePermission,
            "a system administrator must not gain {key} in a chat it never joined"
        );
    }
    assert_eq!(
        status(&base, &format!("/api/chats/{chat_id}/messages"), &admin).await,
        reqwest::StatusCode::FORBIDDEN,
        "the read path let a system administrator into another chat's history"
    );
}

#[tokio::test]
async fn the_creator_stays_authorized_when_the_role_row_is_gone() {
    let (base, state, _task) = start_server().await;
    let owner = session_token(&base, "creator-auth-owner").await;
    let chat_id = create_chat(&base, &owner, "creator-auth-chat").await;
    let owner_user = state
        .session_user(Uuid::parse_str(&owner).unwrap())
        .await
        .unwrap()
        .unwrap();

    assert!(state.is_chat_creator(chat_id, owner_user.id).await.unwrap());
    sqlx::query("DELETE FROM chat_members WHERE room_id = $1 AND user_id = $2")
        .bind(chat_id)
        .bind(owner_user.id)
        .execute(state.pool())
        .await
        .unwrap();
    assert!(
        !state
            .has_chat_role_permission(chat_id, owner_user.id, "room.settings")
            .await
            .unwrap(),
        "the role grant should be gone"
    );
    assert_eq!(
        state
            .authorize_chat_action(chat_id, owner_user.id, "room.settings")
            .await
            .unwrap(),
        ChatAuthorization::Creator,
        "step 2 of the decision order exists so a founder cannot be locked out"
    );
}

#[tokio::test]
async fn a_missing_chat_is_refused_without_consulting_the_chat_type() {
    let (base, state, _task) = start_server().await;
    let user = session_token(&base, "missing-chat-user").await;
    let user = state
        .session_user(Uuid::parse_str(&user).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        state
            .authorize_chat_action(Uuid::new_v4(), user.id, "message.send")
            .await
            .unwrap(),
        ChatAuthorization::NoRolePermission
    );
    assert_eq!(state.chat_type(Uuid::new_v4()).await.unwrap(), None);
}
