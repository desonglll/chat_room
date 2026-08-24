use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{multipart, Client, StatusCode};
use tokio::net::TcpListener;

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31, 0, 5,
    0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

struct TestServer {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn register(client: &Client, base: &str) -> String {
    client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": "avatar-user",
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn image_avatar_can_be_uploaded_read_and_replaced_by_emoji() {
    let server = start_server().await;
    let client = Client::new();
    let token = register(&client, &server.base).await;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(PNG.to_vec())
            .file_name("avatar.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let response = client
        .post(format!("{}/api/users/me/avatar", server.base))
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let user: serde_json::Value = response.json().await.unwrap();
    let avatar_url = user["avatar_emoji"].as_str().unwrap();
    assert!(avatar_url.starts_with("/api/users/"));

    let image = client
        .get(format!("{}{}", server.base, avatar_url))
        .send()
        .await
        .unwrap();
    assert_eq!(image.status(), StatusCode::OK);
    assert_eq!(image.headers()["content-type"], "image/png");
    assert_eq!(image.bytes().await.unwrap().as_ref(), PNG);

    let invalid = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(b"not an image".to_vec()).file_name("fake.png"),
    );
    assert_eq!(
        client
            .post(format!("{}/api/users/me/avatar", server.base))
            .bearer_auth(&token)
            .multipart(invalid)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );

    let updated: serde_json::Value = client
        .patch(format!("{}/api/users/me", server.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "avatar_emoji": "🙂" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["avatar_emoji"], "🙂");
    assert_eq!(
        client
            .get(format!("{}{}", server.base, avatar_url))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}
