//! TG-201: administrators, restrictions, default permissions and the member_count projection,
//! end to end over HTTP and WebSocket, on SQLite always and on PostgreSQL when configured.
//!
//! Each permission is checked at write time (the mutating request is refused) and at read time
//! (what a GET returns follows the viewer's *current* rights, not the rights they had when
//! something was written).

use std::sync::Arc;
use std::time::Duration;

use chat_room::state::AppState;
use chrono::Utc;
use reqwest::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

mod chat_admin_support;

use chat_admin_support::scenarios::{
    default_permissions_scenario, keys, member_count_is_maintained, my_permissions,
};
use chat_admin_support::{next_frame, send_text, serve, with_postgres};

async fn administration_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg201-owner").await;
    let (chat, members) = member_count_is_maintained(&server, &owner).await;
    let (bob, carol) = (&members[0], &members[1]);
    let base = format!("/api/chats/{chat}");

    // A plain member holds the defaults and nothing administrative.
    let bob_rights = my_permissions(&server, &chat, bob).await;
    assert!(bob_rights.contains(&"message.send".to_string()));
    assert!(!bob_rights.contains(&"members.ban".to_string()));

    // Write time: a member can neither restrict, appoint, nor change defaults.
    let restrict_carol = json!({ "denied_permissions": ["message.send"], "until": null });
    for (path, body) in [
        (
            format!("{base}/members/{}/restrictions", carol.id),
            restrict_carol.clone(),
        ),
        (
            format!("{base}/members/{}/admin", carol.id),
            json!({ "permissions": ["members.ban"], "custom_title": "" }),
        ),
        (
            format!("{base}/default-permissions"),
            json!({ "permissions": [] }),
        ),
    ] {
        let (status, _) = server.put(&path, &bob.token, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }

    // The owner appoints Carol with exactly one right and a title.
    let (status, carol_entry) = server
        .put(
            &format!("{base}/members/{}/admin", carol.id),
            &owner.token,
            json!({ "permissions": ["members.ban"], "custom_title": "版主" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{carol_entry}");
    assert_eq!(carol_entry["role"], "admin");
    assert_eq!(carol_entry["custom_title"], "版主");
    let rights = keys(&carol_entry["admin_rights"]);
    assert!(rights.contains(&"members.ban"));
    assert!(
        !rights.contains(&"members.remove"),
        "an explicit selection replaces the default admin rights"
    );
    // Unchecked defaults are really gone at write time.
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("{base}/members/{}", bob.id),
            &carol.token,
            Some(json!({ "action": "remove" })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "members.remove was unchecked"
    );
    // She cannot hand out a right she lacks, nor appoint at all without members.promote.
    let (status, _) = server
        .put(
            &format!("{base}/members/{}/admin", bob.id),
            &carol.token,
            json!({ "permissions": [], "custom_title": "" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Carol restricts Bob from sending for two seconds.
    // Long enough that a loaded CI machine still reads it before it lapses (TG-111 flake).
    let until = Utc::now() + chrono::Duration::seconds(6);
    let (status, bob_entry) = server
        .put(
            &format!("{base}/members/{}/restrictions", bob.id),
            &carol.token,
            json!({ "denied_permissions": ["message.send"], "until": until }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{bob_entry}");
    assert_eq!(
        bob_entry["restrictions"][0]["permission_key"],
        "message.send"
    );
    // An administrator cannot be restricted, and nobody restricts themself.
    let (status, _) = server
        .put(
            &format!("{base}/members/{}/restrictions", carol.id),
            &owner.token,
            restrict_carol.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Write time for the restricted member: the WebSocket send is dropped, sending media and
    // stickers (which imply sending) are denied, while reading keeps working.
    let mut bob_socket = server.socket(&chat, bob).await;
    send_text(&mut bob_socket, "tg201 muted hello").await;
    assert!(!my_permissions(&server, &chat, bob)
        .await
        .contains(&"message.send".to_string()));
    let bob_id = Uuid::parse_str(&bob.id).unwrap();
    let chat_id = Uuid::parse_str(&chat).unwrap();
    assert!(!state
        .has_chat_permission(chat_id, bob_id, "message.send_sticker")
        .await
        .unwrap());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !server
            .history_contains(&chat, &bob.token, "tg201 muted hello")
            .await
    );

    // Read time: Bob sees his own restriction; another member does not.
    let (_, own) = server
        .get(&format!("{base}/members/{}", bob.id), &bob.token)
        .await;
    assert_eq!(own["restrictions"][0]["permission_key"], "message.send");
    let owner_socket_holder = server.socket(&chat, &owner).await;
    let (_, carol_view) = server
        .get(
            &format!("{base}/members/page?filter=restricted"),
            &carol.token,
        )
        .await;
    assert_eq!(carol_view["items"][0]["user_id"], bob.id.as_str());

    // The restriction lifts by itself at `until`, without the sweeper having run.
    let left = (until - Utc::now()).to_std().unwrap_or_default();
    tokio::time::sleep(left + Duration::from_millis(200)).await;
    assert!(my_permissions(&server, &chat, bob)
        .await
        .contains(&"message.send".to_string()));
    send_text(&mut bob_socket, "tg201 free again").await;
    let frame = next_frame(&mut bob_socket, "broadcast").await;
    assert_eq!(frame["content"], "tg201 free again");

    // The sweeper deletes expired rows; the clock is injected rather than slept on.
    let (status, _) = server
        .put(
            &format!("{base}/members/{}/restrictions", bob.id),
            &carol.token,
            json!({
                "denied_permissions": ["message.send_media"],
                "until": Utc::now() + chrono::Duration::hours(1),
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!state
        .has_chat_permission(chat_id, bob_id, "message.send_media")
        .await
        .unwrap());
    let swept = state
        .sweep_expired_chat_restrictions(Utc::now() + chrono::Duration::hours(2))
        .await
        .unwrap();
    assert_eq!(swept, 1);
    assert!(state
        .member_restrictions(chat_id, bob_id, Utc::now())
        .await
        .unwrap()
        .is_empty());
    // Bob's socket learns that his membership changed.
    let frame = next_frame(&mut bob_socket, "member_updated").await;
    assert_eq!(frame["member"]["user_id"], bob.id.as_str());

    // Read time after a demotion: Carol no longer sees restrictions, nor may she write them.
    server
        .put(
            &format!("{base}/members/{}/restrictions", bob.id),
            &carol.token,
            json!({ "denied_permissions": ["message.pin"], "until": null }),
        )
        .await;
    let (status, dismissed) = server
        .call(
            Method::DELETE,
            &format!("{base}/members/{}/admin", carol.id),
            &owner.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dismissed["role"], "member");
    assert_eq!(dismissed["custom_title"], "");
    let (_, page) = server
        .get(&format!("{base}/members/page"), &carol.token)
        .await;
    let bob_row = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["user_id"] == bob.id.as_str())
        .unwrap();
    assert!(bob_row.get("restrictions").is_none(), "{bob_row}");
    let (status, _) = server
        .put(
            &format!("{base}/members/{}/restrictions", bob.id),
            &carol.token,
            json!({ "denied_permissions": [], "until": null }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // The owner still sees it.
    let (_, page) = server
        .get(
            &format!("{base}/members/page?filter=restricted"),
            &owner.token,
        )
        .await;
    assert_eq!(
        page["items"][0]["restrictions"][0]["permission_key"],
        "message.pin"
    );

    default_permissions_scenario(&server, &chat, &owner, bob, carol).await;
    drop(owner_socket_holder);
    drop(bob_socket);
}

#[tokio::test]
async fn chat_administration_on_sqlite() {
    administration_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn chat_administration_on_postgres() {
    with_postgres("chat_administration_on_postgres", administration_scenario).await;
}
