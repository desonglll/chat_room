use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use uuid::Uuid;

struct TestServer {
    base: String,
    state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
struct Account {
    id: Uuid,
    token: String,
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base: format!("http://{address}"),
        state,
        task,
    }
}

async fn register(client: &Client, server: &TestServer, username: &str) -> Account {
    let session: Value = client
        .post(format!("{}/api/users/register", server.base))
        .json(&json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        id: Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
        token: session["token"].as_str().unwrap().into(),
    }
}

async fn create_room(client: &Client, server: &TestServer, owner: &Account) -> Uuid {
    let room: Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&owner.token)
        .json(&json!({
            "name": format!("Tasks {}", Uuid::new_v4().simple()),
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

async fn join(client: &Client, server: &TestServer, room_id: Uuid, member: &Account) {
    let response = client
        .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
        .bearer_auth(&member.token)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
}

async fn insert_message(server: &TestServer, room_id: Uuid, sender: &Account) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
         VALUES (?, ?, ?, 'task-owner', 'Ship the release candidate', ?)",
    )
    .bind(id)
    .bind(room_id)
    .bind(sender.id)
    .bind(Utc::now())
    .execute(server.state.pool())
    .await
    .unwrap();
    id
}

async fn create_task(
    client: &Client,
    server: &TestServer,
    room_id: Uuid,
    creator: &Account,
    assignee_id: Option<Uuid>,
    source_message_id: Option<Uuid>,
) -> Value {
    let response = client
        .post(format!("{}/api/rooms/{room_id}/tasks", server.base))
        .bearer_auth(&creator.token)
        .json(&json!({
            "title": "Prepare release notes",
            "assignee_id": assignee_id,
            "source_message_id": source_message_id,
            "due_at": "2026-09-20T08:00:00Z"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

#[tokio::test]
async fn task_permissions_concurrency_and_source_redaction_are_enforced() {
    let server = start_server().await;
    let client = Client::new();
    let owner = register(&client, &server, "task-owner").await;
    let creator = register(&client, &server, "task-creator").await;
    let assignee = register(&client, &server, "task-assignee").await;
    let viewer = register(&client, &server, "task-viewer").await;
    let outsider = register(&client, &server, "task-outsider").await;
    let room_id = create_room(&client, &server, &owner).await;
    join(&client, &server, room_id, &creator).await;
    join(&client, &server, room_id, &assignee).await;
    join(&client, &server, room_id, &viewer).await;
    let source_id = insert_message(&server, room_id, &owner).await;
    let task = create_task(
        &client,
        &server,
        room_id,
        &creator,
        Some(assignee.id),
        Some(source_id),
    )
    .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["source"]["excerpt"], "Ship the release candidate");
    assert_eq!(task["version"], 1);

    let update = json!({
        "title": "Prepare and review release notes",
        "status": "in_progress",
        "assignee_id": assignee.id,
        "due_at": "2026-09-21T08:00:00Z",
        "version": 1
    });
    let updated: Value = client
        .patch(format!(
            "{}/api/rooms/{room_id}/tasks/{task_id}",
            server.base
        ))
        .bearer_auth(&assignee.token)
        .json(&update)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["version"], 2);
    assert_eq!(updated["status"], "in_progress");

    for (account, expected) in [
        (&creator, StatusCode::CONFLICT),
        (&viewer, StatusCode::FORBIDDEN),
        (&outsider, StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            client
                .patch(format!(
                    "{}/api/rooms/{room_id}/tasks/{task_id}",
                    server.base
                ))
                .bearer_auth(&account.token)
                .json(&update)
                .send()
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    assert_eq!(
        client
            .get(format!("{}/api/rooms/{room_id}/tasks", server.base))
            .bearer_auth(&outsider.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .delete(format!(
                "{}/api/rooms/{room_id}/tasks/{task_id}",
                server.base
            ))
            .bearer_auth(&outsider.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(source_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    client
        .patch(format!(
            "{}/api/rooms/{room_id}/members/{}",
            server.base, assignee.id
        ))
        .bearer_auth(&owner.token)
        .json(&json!({ "action": "remove" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let tasks: Vec<Value> = client
        .get(format!("{}/api/rooms/{room_id}/tasks", server.base))
        .bearer_auth(&owner.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!tasks[0]["assignee_active"].as_bool().unwrap());
    assert_eq!(tasks[0]["source"]["recalled"], true);
    assert_eq!(tasks[0]["source"]["excerpt"], "");
    assert_eq!(
        client
            .patch(format!(
                "{}/api/rooms/{room_id}/tasks/{task_id}",
                server.base
            ))
            .bearer_auth(&assignee.token)
            .json(&json!({
                "title": "Prepare and review release notes",
                "status": "done",
                "assignee_id": assignee.id,
                "due_at": "2026-09-21T08:00:00Z",
                "version": 2
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn admins_manage_tasks_and_room_retention_hides_then_purges_them() {
    let server = start_server().await;
    let client = Client::new();
    let owner = register(&client, &server, "task-delete-owner").await;
    let admin = register(&client, &server, "task-delete-admin").await;
    let room_id = create_room(&client, &server, &owner).await;
    join(&client, &server, room_id, &admin).await;
    client
        .patch(format!(
            "{}/api/rooms/{room_id}/members/{}",
            server.base, admin.id
        ))
        .bearer_auth(&owner.token)
        .json(&json!({ "action": "set_role", "role": "admin" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let disposable = create_task(&client, &server, room_id, &owner, None, None).await;
    assert_eq!(
        client
            .delete(format!(
                "{}/api/rooms/{room_id}/tasks/{}",
                server.base,
                disposable["id"].as_str().unwrap()
            ))
            .bearer_auth(&admin.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    create_task(&client, &server, room_id, &owner, Some(admin.id), None).await;
    assert_eq!(
        client
            .delete(format!("{}/api/rooms/{room_id}", server.base))
            .bearer_auth(&owner.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client
            .get(format!("{}/api/rooms/{room_id}/tasks", server.base))
            .bearer_auth(&owner.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let retained: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_tasks WHERE room_id = ?")
        .bind(room_id)
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(retained, 1, "soft-deleted rooms retain tasks until purge");
    sqlx::query("DELETE FROM rooms WHERE id = ?")
        .bind(room_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let purged: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM room_tasks WHERE room_id = ?")
        .bind(room_id)
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(purged, 0, "hard room purge cascades to tasks");
}
