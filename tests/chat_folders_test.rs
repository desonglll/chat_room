//! TG-501 chat folders over HTTP: create / list / replace / reorder / delete, the 10-folder and
//! title limits, only readable chats may be named, and chats the owner leaves drop out.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres};

async fn chat_folders_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("cf-alice").await;
    let bob = server.register("cf-bob").await;
    let mine = server.create_group(&alice, "cf-mine").await;
    let joined = server.create_group(&bob, "cf-joined").await;
    server.join(&joined, &alice).await;
    let foreign = server.create_group(&bob, "cf-foreign").await;
    let create = |body: Value| {
        let server = &server;
        let token = alice.token.clone();
        async move {
            server
                .call(Method::POST, "/api/users/me/folders", &token, Some(body))
                .await
        }
    };

    let (status, work) = create(json!({
        "title": "工作", "emoji": "💼", "include_types": ["groups"],
        "exclude_chat_ids": [joined], "exclude_muted": true
    }))
    .await;
    assert_eq!(status, StatusCode::CREATED, "{work}");
    assert_eq!(work["include_types"], json!(["groups"]));
    assert_eq!(work["exclude_chat_ids"], json!([joined]));
    let (status, _) = create(json!({ "title": "个人", "include_chat_ids": [mine, joined] })).await;
    assert_eq!(status, StatusCode::CREATED);

    // Validation: a title over 12 characters, an unknown type, an empty folder, a foreign chat.
    let (status, _) =
        create(json!({ "title": "这是一个特别特别长的文件夹名称", "include_types": ["groups"] }))
            .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = create(json!({ "title": "x", "include_types": ["bots"] })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = create(json!({ "title": "x" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = create(json!({ "title": "x", "include_chat_ids": [foreign] })).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a chat the owner cannot read"
    );

    // At most 10 folders.
    for index in 0..8 {
        let (status, _) =
            create(json!({ "title": format!("f{index}"), "include_types": ["private"] })).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, _) = create(json!({ "title": "eleven", "include_types": ["private"] })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Reorder: exactly the caller's folders.
    let (_, folders) = server.get("/api/users/me/folders", &alice.token).await;
    let mut ids: Vec<Value> = folders
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].clone())
        .collect();
    assert_eq!(ids.len(), 10);
    ids.reverse();
    let (status, reordered) = server
        .call(
            Method::PUT,
            "/api/users/me/folders/order",
            &alice.token,
            Some(json!({ "folder_ids": ids })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reordered[0]["id"], ids[0]);
    let (status, _) = server
        .call(
            Method::PUT,
            "/api/users/me/folders/order",
            &alice.token,
            Some(json!({ "folder_ids": [ids[0]] })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Leaving a chat drops it from the folder's lists.
    let personal = reordered
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["title"] == "个人")
        .unwrap()
        .clone();
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/chats/{joined}/members/me"),
            &alice.token,
            None,
        )
        .await;
    assert!(status.is_success(), "leave: {status}");
    let (_, folders) = server.get("/api/users/me/folders", &alice.token).await;
    let personal_now = folders
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == personal["id"])
        .unwrap()
        .clone();
    assert_eq!(personal_now["include_chat_ids"], json!([mine]));

    // Bob cannot touch Alice's folder; Alice can replace and delete it.
    let path = format!("/api/users/me/folders/{}", personal["id"].as_str().unwrap());
    let (status, _) = server.call(Method::DELETE, &path, &bob.token, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, replaced) = server
        .call(
            Method::PUT,
            &path,
            &alice.token,
            Some(json!({ "title": "家", "include_types": ["private"] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["include_chat_ids"], json!([]));
    let (status, _) = server.call(Method::DELETE, &path, &alice.token, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, folders) = server.get("/api/users/me/folders", &alice.token).await;
    assert_eq!(folders.as_array().unwrap().len(), 9);
}

#[tokio::test]
async fn sqlite_chat_folders_crud_limits_and_authorization() {
    chat_folders_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_chat_folders_crud_limits_and_authorization() {
    with_postgres("postgres_chat_folders", chat_folders_scenario).await;
}
