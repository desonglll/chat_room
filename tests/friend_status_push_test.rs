//! TG-1102: the account socket pushes `friend_statuses` when a friend's presence changes, under
//! the same last_seen rules as `GET /api/friends/statuses`.

mod privacy_support;

use futures_util::{SinkExt, StreamExt};
use privacy_support::{start_server, Account, TestServer};
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn account_socket(server: &TestServer, account: &Account) -> Socket {
    let url = format!("{}/ws/account", server.base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(json!({ "token": account.token }).to_string()))
        .await
        .unwrap();
    socket
}

/// The next `friend_statuses` frame whose entry for `friend` satisfies `wanted`.
async fn wait_for(socket: &mut Socket, friend: &Account, wanted: impl Fn(&Value) -> bool) -> Value {
    timeout(Duration::from_secs(10), async {
        loop {
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                continue;
            };
            let frame: Value = serde_json::from_str(&text).unwrap();
            if frame["type"] != "friend_statuses" {
                continue;
            }
            let status = frame["statuses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["user_id"] == friend.id.to_string())
                .map(|entry| entry["status"].clone())
                .unwrap_or(Value::Null);
            if wanted(&status) {
                return status;
            }
        }
    })
    .await
    .expect("no matching friend_statuses frame within 10 s")
}

#[tokio::test]
async fn friend_presence_is_pushed_and_respects_privacy() {
    let server = start_server().await;
    let alice = server.account("fp-alice").await;
    let bob = server.account("fp-bob").await;
    server.befriend(&alice, &bob).await;
    let mut feed = account_socket(&server, &alice).await;

    let chat = server.create_group(&bob, "fp-chat").await;
    let (socket, _) = server.open_chat(chat, &bob).await;
    wait_for(&mut feed, &bob, |status| status["kind"] == "online").await;

    server.close_and_settle(chat, &bob, socket).await;
    let offline = wait_for(&mut feed, &bob, |status| status["kind"] == "offline").await;
    assert!(offline["last_seen"].is_string());

    // Bob hides his last seen: the next push carries only the obscured tier.
    server.put_rule(&bob, "last_seen", "nobody", &[], &[]).await;
    let hidden = wait_for(&mut feed, &bob, |status| status["kind"] != "offline").await;
    assert!(hidden.get("last_seen").is_none(), "{hidden}");
    assert_ne!(hidden["kind"], "online");
}
