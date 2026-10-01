//! The Web Push payload for a notification: title, optional body (only when the device opted
//! into details), and the URL the click opens. TG-601: URLs are the React client's routes.

use super::models::PushPayload;
use crate::notifications::{NotificationKind, NotificationView};

pub(super) fn payload_for(notification: &NotificationView, show_details: bool) -> PushPayload {
    // TG-601: the React client's routes. A chat notification opens the chat at the message
    // (`/chat/:id?message=` jumps there); anything without a chat opens the app.
    let url = match notification.kind {
        NotificationKind::FriendRequest | NotificationKind::AiRunCompleted => "/".into(),
        _ => notification
            .room_id
            .map(|room_id| {
                notification
                    .message_id
                    .map(|message_id| format!("/chat/{room_id}?message={message_id}"))
                    .unwrap_or_else(|| format!("/chat/{room_id}"))
            })
            .unwrap_or_else(|| "/".into()),
    };
    PushPayload {
        title: "Echo Gate".into(),
        body: show_details.then(|| notification.summary.clone()),
        url,
        tag: format!("notification:{}", notification.id),
        silent: false,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn a_chat_notification_opens_the_message_in_the_react_client() {
        let room_id = uuid::Uuid::new_v4();
        let message_id = uuid::Uuid::new_v4();
        let view = |kind, room_id, message_id| crate::notifications::NotificationView {
            id: "n1".into(),
            kind,
            actor: None,
            room_id,
            room_name: None,
            message_id,
            run_id: None,
            summary: "s".into(),
            source_available: true,
            created_at: Utc::now(),
            read_at: None,
        };
        let mention = payload_for(
            &view(NotificationKind::Mention, Some(room_id), Some(message_id)),
            true,
        );
        assert_eq!(mention.url, format!("/chat/{room_id}?message={message_id}"));
        let chat = payload_for(&view(NotificationKind::Mention, Some(room_id), None), false);
        assert_eq!(chat.url, format!("/chat/{room_id}"));
        assert!(chat.body.is_none(), "no details unless the device opted in");
        let friend = payload_for(&view(NotificationKind::FriendRequest, None, None), true);
        assert_eq!(friend.url, "/");
    }
}
