use chrono::Utc;
use uuid::Uuid;

use crate::{models::Room, state::AppState};

#[tokio::test]
async fn sqlite_index_jobs_accept_text_and_blob_uuid_storage() {
    let state = AppState::new().await.unwrap();
    let text_id = Uuid::new_v4();
    let blob_id = Uuid::new_v4();
    for (message_id, bind_as_text) in [(text_id, true), (blob_id, false)] {
        let mut query = sqlx::query(
            "INSERT INTO message_index_outbox \
             (message_id, operation, next_attempt_at, updated_at) \
             VALUES ($1, 'delete', $2, $2)",
        );
        query = if bind_as_text {
            query.bind(message_id.to_string())
        } else {
            query.bind(message_id)
        };
        query.bind(Utc::now()).execute(state.pool()).await.unwrap();
    }

    let jobs = state.ready_index_jobs().await.unwrap();
    assert_eq!(jobs.len(), 2);
    let text_job = jobs.iter().find(|job| job.message_id == text_id).unwrap();
    let blob_job = jobs.iter().find(|job| job.message_id == blob_id).unwrap();

    state.complete_index_job(text_job).await.unwrap();
    state
        .retry_index_job(blob_job, "temporary failure")
        .await
        .unwrap();
    let remaining: (i64, String) =
        sqlx::query_as("SELECT attempt_count, last_error FROM message_index_outbox")
            .fetch_one(state.pool())
            .await
            .unwrap();
    assert_eq!(remaining, (1, "temporary failure".into()));
}

#[tokio::test]
async fn retrieved_messages_are_rechecked_against_membership_and_recall_state() {
    let state = AppState::new().await.unwrap();
    let owner = state.insert_user("index-owner", "unused").await.unwrap();
    let outsider = state.insert_user("index-outsider", "unused").await.unwrap();
    let room = Room {
        id: Uuid::new_v4(),
        name: "indexed room".into(),
        password_hash: String::new(),
        has_password: false,
        creator_user_id: Some(owner.id),
        join_policy: "open".into(),
        avatar_emoji: String::new(),
        description: String::new(),
        membership_status: None,
        membership_role: None,
        unread_count: 0,
        created_at: Utc::now(),
    };
    state
        .create_room_with_owner(room.clone(), owner.id)
        .await
        .unwrap();
    let stored = state
        .store_message(
            room.id,
            owner.id,
            &owner.username,
            "",
            "private release plan",
            None,
            Some(Uuid::new_v4()),
        )
        .await
        .unwrap()
        .message;
    let newer = state
        .store_message(
            room.id,
            owner.id,
            &owner.username,
            "",
            "newer indexed detail",
            None,
            Some(Uuid::new_v4()),
        )
        .await
        .unwrap()
        .message;

    let visible = state
        .authorized_retrieved_messages(owner.id, room.id, &[newer.id, stored.id])
        .await
        .unwrap();
    assert_eq!(
        visible.iter().map(|message| message.id).collect::<Vec<_>>(),
        [newer.id, stored.id]
    );
    assert!(state
        .authorized_retrieved_messages(outsider.id, room.id, &[stored.id])
        .await
        .unwrap()
        .is_empty());

    state
        .recall_message(room.id, owner.id, stored.id)
        .await
        .unwrap();
    assert!(state
        .authorized_retrieved_messages(owner.id, room.id, &[stored.id])
        .await
        .unwrap()
        .is_empty());
}
