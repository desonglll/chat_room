//! Account-scoped unread counters and cross-room message events.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
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
    pending_join_requests: i64,
    pending_join_requested_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct UnreadSnapshot {
    #[serde(rename = "type")]
    kind: &'static str,
    rooms: Vec<RoomAccountState>,
}

#[derive(Serialize)]
struct SocialChanged {
    #[serde(rename = "type")]
    kind: &'static str,
    incoming_request_count: usize,
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
    let mut message_cursor = match state.latest_account_message_cursor(user.id).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::warn!("initialize account message cursor failed: {error}");
            return;
        }
    };

    let mut previous = None;
    let mut previous_social = None;
    let mut refresh = interval(Duration::from_millis(750));
    refresh.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        if state.maintenance_active() {
            return;
        }
        tokio::select! {
            _ = refresh.tick() => {
                let events = match state.account_messages_after(user.id, message_cursor.as_ref()).await {
                    Ok(events) => events,
                    Err(error) => {
                        tracing::warn!("load account message events failed: {error}");
                        continue;
                    }
                };
                for (cursor, event) in events {
                    message_cursor = Some(cursor);
                    if event.sender_id == Some(user.id) {
                        continue;
                    }
                    let Ok(json) = serde_json::to_string(&event) else { continue };
                    if socket.send(Message::Text(json)).await.is_err() {
                        return;
                    }
                }
                let unread = match state.room_unread_counts(user.id).await {
                    Ok(rows) => rows.into_iter().collect::<std::collections::HashMap<_, _>>(),
                    Err(error) => {
                        tracing::warn!("load live unread counts failed: {}", error);
                        continue;
                    }
                };
                let counts = match state.account_membership_states(user.id).await {
                    Ok(rows) => rows.into_iter().map(|(room_id, membership_status, membership_role, pending_join_requests, pending_join_requested_at)| RoomAccountState {
                        room_id,
                        unread_count: unread.get(&room_id).copied().unwrap_or(0),
                        membership_status,
                        membership_role,
                        pending_join_requests,
                        pending_join_requested_at,
                    }).collect::<Vec<_>>(),
                    Err(error) => {
                        tracing::warn!("load live membership states failed: {}", error);
                        continue;
                    }
                };
                if previous.as_ref() != Some(&counts) {
                    previous = Some(counts.clone());
                    let payload = UnreadSnapshot { kind: "unread_counts", rooms: counts };
                    let Ok(json) = serde_json::to_string(&payload) else { continue };
                    if socket.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                let social = match state.social_account_state(user.id).await {
                    Ok(social) => social,
                    Err(error) => {
                        tracing::warn!("load social account state failed: {error}");
                        continue;
                    }
                };
                if previous_social.as_ref() != Some(&social.fingerprint) {
                    previous_social = Some(social.fingerprint);
                    let payload = SocialChanged {
                        kind: "social_changed",
                        incoming_request_count: social.incoming_request_count,
                    };
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
