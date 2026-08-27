use std::str::FromStr;

use chat_room::{
    notifications::{NotificationCursor, NotificationEvent, NotificationKind, NotificationQuery},
    state::AppState,
};
use chrono::{Duration, Utc};

#[tokio::test]
async fn notification_domain_interface_is_idempotent_paginated_and_read_scoped() {
    let state = AppState::new().await.unwrap();
    let recipient = state
        .insert_user("notification-store-recipient", "unused")
        .await
        .unwrap();
    let actor = state
        .insert_user("notification-store-actor", "unused")
        .await
        .unwrap();
    let now = Utc::now();
    for index in 0..3 {
        let event = NotificationEvent {
            recipient_id: recipient.id,
            kind: NotificationKind::FriendRequest,
            actor_id: Some(actor.id),
            room_id: None,
            message_id: None,
            run_id: None,
            summary: String::new(),
            dedupe_key: format!("domain-notification-{index}"),
            created_at: now + Duration::milliseconds(index),
        };
        assert!(state.record_notification(&event).await.unwrap());
        assert!(!state.record_notification(&event).await.unwrap());
    }
    assert_eq!(
        state.notification_unread_count(recipient.id).await.unwrap(),
        3
    );

    let first = state
        .list_notifications(
            recipient.id,
            &NotificationQuery {
                kind: None,
                cursor: None,
                limit: 2,
            },
        )
        .await
        .unwrap();
    assert_eq!(first.items.len(), 2);
    let cursor = NotificationCursor::from_str(first.next_cursor.as_deref().unwrap()).unwrap();
    let second = state
        .list_notifications(
            recipient.id,
            &NotificationQuery {
                kind: None,
                cursor: Some(cursor),
                limit: 2,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert!(second.next_cursor.is_none());

    assert!(state
        .mark_notification_read(recipient.id, &first.items[0].id)
        .await
        .unwrap());
    assert_eq!(
        state.notification_unread_count(recipient.id).await.unwrap(),
        2
    );
    assert_eq!(
        state
            .mark_all_notifications_read(recipient.id)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        state.notification_unread_count(recipient.id).await.unwrap(),
        0
    );
}
