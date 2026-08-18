//! Account-scoped live unread counters across all joined rooms.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::time::{interval, timeout, MissedTickBehavior};
use uuid::Uuid;

use crate::state::SharedState;

#[derive(Deserialize)]
struct AccountAuth {
    token: Uuid,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
struct RoomAccountState {
    room_id: Uuid,
    unread_count: i64,
    membership_status: String,
    membership_role: String,
}

#[derive(Serialize)]
struct UnreadSnapshot {
    #[serde(rename = "type")]
    kind: &'static str,
    rooms: Vec<RoomAccountState>,
}

pub async fn account_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_account_socket(socket, state))
}

async fn handle_account_socket(mut socket: WebSocket, state: SharedState) {
    let raw = match timeout(Duration::from_secs(10), socket.recv()).await {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => return,
    };
    let Ok(auth) = serde_json::from_str::<AccountAuth>(&raw) else {
        return;
    };
    let user = match state.session_user(auth.token).await {
        Ok(Some(user)) => user,
        _ => return,
    };

    let mut previous = Vec::new();
    let mut refresh = interval(Duration::from_millis(750));
    refresh.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = refresh.tick() => {
                let unread = match state.room_unread_counts(user.id).await {
                    Ok(rows) => rows.into_iter().collect::<std::collections::HashMap<_, _>>(),
                    Err(error) => {
                        tracing::warn!("load live unread counts failed: {}", error);
                        continue;
                    }
                };
                let counts = match state.account_membership_states(user.id).await {
                    Ok(rows) => rows.into_iter().map(|(room_id, membership_status, membership_role)| RoomAccountState {
                        room_id,
                        unread_count: unread.get(&room_id).copied().unwrap_or(0),
                        membership_status,
                        membership_role,
                    }).collect::<Vec<_>>(),
                    Err(error) => {
                        tracing::warn!("load live membership states failed: {}", error);
                        continue;
                    }
                };
                if counts != previous {
                    previous = counts.clone();
                    let payload = UnreadSnapshot { kind: "unread_counts", rooms: counts };
                    let Ok(json) = serde_json::to_string(&payload) else { continue };
                    if socket.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            frame = socket.recv() => match frame {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            }
        }
    }
}
