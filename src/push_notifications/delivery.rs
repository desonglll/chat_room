use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use async_trait::async_trait;

use super::{
    models::{ClaimedPushJob, PushPayload},
    ProductionPushSender,
};
use crate::{notifications::NotificationKind, state::AppState};

const BATCH_SIZE: i64 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PushSendOutcome {
    Delivered,
    Expired,
    Retryable,
}

#[async_trait]
pub(crate) trait PushSender: Send + Sync {
    async fn send(&self, job: &ClaimedPushJob, payload: &PushPayload) -> PushSendOutcome;
}

pub(crate) fn ensure_dispatcher(state: Arc<AppState>) {
    if !state.web_push_config().enabled
        || state.push_dispatcher_started.swap(true, Ordering::AcqRel)
    {
        return;
    }
    let sender = match ProductionPushSender::new(state.web_push_config()) {
        Ok(sender) => Arc::new(sender),
        Err(error) => {
            state
                .push_dispatcher_started
                .store(false, Ordering::Release);
            tracing::error!("initialize Web Push sender failed: {error:#}");
            return;
        }
    };
    tokio::spawn(dispatch_loop(state, sender));
}

async fn dispatch_loop(state: Arc<AppState>, sender: Arc<dyn PushSender>) {
    let interval = Duration::from_millis(state.web_push_config().poll_interval_ms);
    loop {
        if let Err(error) = dispatch_batch(&state, sender.as_ref()).await {
            tracing::warn!("Web Push dispatch batch failed: {error}");
        }
        tokio::time::sleep(interval).await;
    }
}

pub(crate) async fn dispatch_batch(
    state: &AppState,
    sender: &dyn PushSender,
) -> Result<usize, sqlx::Error> {
    let (claim_token, jobs) = state.claim_push_jobs(BATCH_SIZE).await?;
    let count = jobs.len();
    for job in jobs {
        let Some(notification) = state
            .notification_for_push(job.recipient_id, &job.notification_id)
            .await?
        else {
            state.complete_push_job(&job.id, &claim_token).await?;
            continue;
        };
        if !notification.source_available {
            state.complete_push_job(&job.id, &claim_token).await?;
            continue;
        }
        if let Some(room_id) = notification.room_id {
            let mention_or_reply = matches!(
                notification.kind,
                NotificationKind::Mention | NotificationKind::Reply
            );
            if !state
                .room_allows_push(job.recipient_id, room_id, mention_or_reply)
                .await?
            {
                state.complete_push_job(&job.id, &claim_token).await?;
                continue;
            }
        }
        let payload = payload_for(&notification, job.show_details);
        match sender.send(&job, &payload).await {
            PushSendOutcome::Delivered => state.complete_push_job(&job.id, &claim_token).await?,
            PushSendOutcome::Expired => {
                state.expire_push_subscription(&job.subscription_id).await?
            }
            PushSendOutcome::Retryable => {
                state
                    .retry_push_job(&job, &claim_token, state.web_push_config().max_attempts)
                    .await?
            }
        }
    }
    Ok(count)
}

fn payload_for(
    notification: &crate::notifications::NotificationView,
    show_details: bool,
) -> PushPayload {
    let url = match notification.kind {
        NotificationKind::FriendRequest => "/contacts".into(),
        NotificationKind::AiRunCompleted => notification
            .run_id
            .map(|id| format!("/assistant?run={id}"))
            .unwrap_or_else(|| "/assistant".into()),
        _ => notification
            .room_id
            .map(|room_id| {
                notification
                    .message_id
                    .map(|message_id| format!("/rooms/{room_id}?message={message_id}"))
                    .unwrap_or_else(|| format!("/rooms/{room_id}"))
            })
            .unwrap_or_else(|| "/notifications".into()),
    };
    PushPayload {
        title: "Echo Gate".into(),
        body: show_details.then(|| notification.summary.clone()),
        url,
        tag: format!("notification:{}", notification.id),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::Utc;

    use super::*;
    use crate::{
        models::Room,
        notifications::{NotificationEvent, NotificationKind},
        push_notifications::{PushSubscriptionKeys, SavePushSubscriptionRequest},
    };

    struct RecordingSender {
        outcome: PushSendOutcome,
        payloads: Mutex<Vec<PushPayload>>,
    }

    #[async_trait]
    impl PushSender for RecordingSender {
        async fn send(&self, _job: &ClaimedPushJob, payload: &PushPayload) -> PushSendOutcome {
            self.payloads.lock().unwrap().push(payload.clone());
            self.outcome
        }
    }

    async fn fixture() -> (AppState, uuid::Uuid, uuid::Uuid) {
        let state = AppState::new().await.unwrap();
        let recipient = state.insert_user("push-recipient", "unused").await.unwrap();
        let actor = state.insert_user("push-actor", "unused").await.unwrap();
        (state, recipient.id, actor.id)
    }

    async fn subscribe(state: &AppState, user_id: uuid::Uuid, endpoint: &str, details: bool) {
        state
            .save_push_subscription(
                user_id,
                &SavePushSubscriptionRequest {
                    endpoint: endpoint.into(),
                    keys: PushSubscriptionKeys {
                        p256dh: "public-key".into(),
                        auth: "auth-key".into(),
                    },
                    show_details: details,
                },
            )
            .await
            .unwrap()
            .unwrap();
    }

    async fn notify(state: &AppState, recipient_id: uuid::Uuid, actor_id: uuid::Uuid, key: &str) {
        state
            .record_notification(&NotificationEvent {
                recipient_id,
                kind: NotificationKind::FriendRequest,
                actor_id: Some(actor_id),
                room_id: None,
                message_id: None,
                run_id: None,
                summary: "Minimal private summary".into(),
                dedupe_key: key.into(),
                created_at: Utc::now(),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn dispatches_each_device_with_its_own_detail_preference() {
        let (state, recipient_id, actor_id) = fixture().await;
        subscribe(&state, recipient_id, "https://push.example/first", false).await;
        subscribe(&state, recipient_id, "https://push.example/second", true).await;
        notify(&state, recipient_id, actor_id, "push-device-details").await;
        let sender = RecordingSender {
            outcome: PushSendOutcome::Delivered,
            payloads: Mutex::new(Vec::new()),
        };

        assert_eq!(dispatch_batch(&state, &sender).await.unwrap(), 2);
        {
            let payloads = sender.payloads.lock().unwrap();
            assert_eq!(payloads.len(), 2);
            assert_eq!(
                payloads
                    .iter()
                    .filter(|payload| payload.body.is_some())
                    .count(),
                1
            );
            assert!(payloads.iter().all(|payload| payload.url == "/contacts"));
        }
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM push_delivery_jobs")
            .fetch_one(state.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn removes_an_expired_device_subscription() {
        let (state, recipient_id, actor_id) = fixture().await;
        subscribe(&state, recipient_id, "https://push.example/expired", false).await;
        notify(&state, recipient_id, actor_id, "push-expired").await;
        let sender = RecordingSender {
            outcome: PushSendOutcome::Expired,
            payloads: Mutex::new(Vec::new()),
        };

        assert_eq!(dispatch_batch(&state, &sender).await.unwrap(), 1);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM push_subscriptions")
            .fetch_one(state.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn room_preferences_gate_delivery_at_send_time() {
        let (state, recipient_id, _) = fixture().await;
        let room_id = uuid::Uuid::new_v4();
        let created_at = Utc::now();
        state
            .create_room_with_owner(
                Room {
                    id: room_id,
                    name: "push-room".into(),
                    password_hash: String::new(),
                    has_password: false,
                    creator_user_id: Some(recipient_id),
                    join_policy: "open".into(),
                    avatar_emoji: String::new(),
                    description: String::new(),
                    membership_status: None,
                    membership_role: None,
                    unread_count: 0,
                    created_at,
                },
                recipient_id,
            )
            .await
            .unwrap();

        assert!(state
            .room_allows_push(recipient_id, room_id, false)
            .await
            .unwrap());
        sqlx::query(
            "UPDATE room_memberships SET notification_level = 'mentions' \
             WHERE room_id = $1 AND user_id = $2",
        )
        .bind(room_id)
        .bind(recipient_id)
        .execute(state.pool())
        .await
        .unwrap();
        assert!(!state
            .room_allows_push(recipient_id, room_id, false)
            .await
            .unwrap());
        assert!(state
            .room_allows_push(recipient_id, room_id, true)
            .await
            .unwrap());

        sqlx::query(
            "UPDATE room_memberships SET muted_until = $1 WHERE room_id = $2 AND user_id = $3",
        )
        .bind(created_at + chrono::Duration::hours(1))
        .bind(room_id)
        .bind(recipient_id)
        .execute(state.pool())
        .await
        .unwrap();
        assert!(!state
            .room_allows_push(recipient_id, room_id, true)
            .await
            .unwrap());

        sqlx::query("DELETE FROM room_memberships WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(recipient_id)
            .execute(state.pool())
            .await
            .unwrap();
        assert!(!state
            .room_allows_push(recipient_id, room_id, true)
            .await
            .unwrap());
    }
}
