//! Background execution of TG-1205 messaging actions (scheduled sends, search, invite links).

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::client_api::ApiClient;

use super::messaging::{local_time, MessagingAction, MessagingEvent};
use super::model::AppEvent;

pub fn run(
    action: MessagingAction,
    server: String,
    token: Option<Uuid>,
    sender: mpsc::UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        let api = ApiClient::new(&server, token);
        let event = match action {
            MessagingAction::LoadScheduled(room_id) => MessagingEvent::Scheduled {
                room_id,
                result: api.scheduled_messages(room_id).await,
            },
            MessagingAction::Schedule {
                room_id,
                content,
                at,
                silent,
            } => MessagingEvent::ScheduledChanged {
                room_id,
                result: api
                    .schedule_message(room_id, &content, at, silent)
                    .await
                    .map(|scheduled| {
                        format!(
                            "Scheduled for {} (S lists scheduled messages)",
                            local_time(&scheduled.scheduled_at)
                        )
                    }),
            },
            MessagingAction::SendScheduledNow { room_id, id } => MessagingEvent::ScheduledChanged {
                room_id,
                result: api
                    .send_scheduled_now(room_id, id)
                    .await
                    .map(|()| "Scheduled message sent".to_string()),
            },
            MessagingAction::CancelScheduled { room_id, id } => MessagingEvent::ScheduledChanged {
                room_id,
                result: api
                    .cancel_scheduled(room_id, id)
                    .await
                    .map(|()| "Scheduled message cancelled".to_string()),
            },
            MessagingAction::Search {
                room_id,
                password,
                query,
            } => {
                let result = api.search_chat(room_id, password.as_deref(), &query).await;
                MessagingEvent::Found { query, result }
            }
            MessagingAction::JoinInvite(token) => {
                MessagingEvent::Joined(api.join_invite(&token).await)
            }
        };
        let _ = sender.send(AppEvent::Messaging(event));
    });
}
