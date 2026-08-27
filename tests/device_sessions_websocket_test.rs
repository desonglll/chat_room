use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::TcpListener,
    time::{timeout, Duration},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn start_server() -> (String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    (format!("http://{address}"), task)
}

async fn authenticate(client: &reqwest::Client, base: &str, endpoint: &str) -> String {
    client
        .post(format!("{base}/api/users/{endpoint}"))
        .json(&serde_json::json!({
            "username": "session-websocket-user",
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
        .to_owned()
}

#[tokio::test]
async fn revoked_device_websockets_disconnect_promptly() {
    let (base, task) = start_server().await;
    let client = reqwest::Client::new();
    let revoked_token = authenticate(&client, &base, "register").await;
    let room = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&revoked_token)
        .json(&serde_json::json!({ "name": "session-room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let websocket_base = base.replace("http://", "ws://");
    let (mut room_socket, _) = connect_async(format!(
        "{websocket_base}/ws/{}",
        room["id"].as_str().unwrap()
    ))
    .await
    .unwrap();
    room_socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": revoked_token }).to_string(),
        ))
        .await
        .unwrap();
    while let Some(Ok(Message::Text(text))) = room_socket.next().await {
        if serde_json::from_str::<serde_json::Value>(&text).unwrap()["type"] == "history_complete" {
            break;
        }
    }

    let (mut account_socket, _) = connect_async(format!("{websocket_base}/ws/account"))
        .await
        .unwrap();
    account_socket
        .send(Message::Text(
            serde_json::json!({ "token": revoked_token }).to_string(),
        ))
        .await
        .unwrap();
    timeout(Duration::from_secs(2), account_socket.next())
        .await
        .expect("account socket should authenticate")
        .expect("account socket should stay open before revocation")
        .unwrap();

    let current_token = authenticate(&client, &base, "login").await;
    let sessions = client
        .get(format!("{base}/api/users/me/sessions"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    let revoked_id = sessions
        .iter()
        .find(|session| session["current"] == false)
        .unwrap()["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        client
            .delete(format!("{base}/api/users/me/sessions/{revoked_id}"))
            .bearer_auth(current_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_socket_closes(&mut room_socket).await;
    assert_socket_closes(&mut account_socket).await;
    task.abort();
}

async fn assert_socket_closes(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                None | Some(Ok(Message::Close(_))) | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .expect("revoked device socket should close promptly");
}
