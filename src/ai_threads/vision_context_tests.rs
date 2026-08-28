use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use super::{bounded_visual_context, VisualEvidence};
use crate::ai::{VisionLimits, VisualProjection};
use crate::ai_threads::{AiCitationAttachment, AiCitationSource};
use crate::models::Room;
use crate::state::AppState;

use super::super::run_store::AiRunExecution;
use super::super::vision_store::{
    load_cached_projection, read_authorized_image, store_visual_projection,
};

#[tokio::test]
async fn image_bytes_are_loaded_only_while_room_membership_is_active() {
    let state = Arc::new(AppState::new().await.unwrap());
    let owner = state.insert_user("vision-owner", "unused").await.unwrap();
    let room = Room {
        id: Uuid::new_v4(),
        name: "Vision room".into(),
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
    let attachment_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let bytes = b"private-image-bytes";
    state
        .attachment_store()
        .import_legacy(attachment_id, bytes)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO attachments \
         (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, created_at) \
         VALUES (?, ?, ?, ?, 'private.png', 'image/png', ?, ?)",
    )
    .bind(attachment_id)
    .bind(Uuid::new_v4())
    .bind(room.id)
    .bind(owner.id)
    .bind(bytes.len() as i64)
    .bind(Utc::now())
    .execute(state.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, attachment_id, created_at) \
         VALUES (?, ?, ?, 'vision-owner', 'release screenshot', ?, ?)",
    )
    .bind(message_id)
    .bind(room.id)
    .bind(owner.id)
    .bind(attachment_id)
    .bind(Utc::now())
    .execute(state.pool())
    .await
    .unwrap();
    let execution = execution(owner.id, room.id);
    let source = source(room.id, message_id, attachment_id, bytes.len() as i64);
    let limits = VisionLimits {
        max_images: 1,
        max_total_images: 1,
        max_image_bytes: 1024,
    };

    let loaded = read_authorized_image(&state, &execution, &source, limits)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.bytes, bytes);

    let projection = VisualProjection {
        summary: "Private release screenshot".into(),
        visible_text: vec!["Launch Friday".into()],
        key_facts: vec!["Release is Friday".into()],
        uncertainties: Vec::new(),
    };
    assert!(
        store_visual_projection(&state, &execution, &source, "vision-v1", 1, &projection,)
            .await
            .unwrap()
    );
    assert_eq!(
        load_cached_projection(&state, &execution, &source, "vision-v1", 1)
            .await
            .unwrap(),
        Some(projection)
    );
    assert!(
        load_cached_projection(&state, &execution, &source, "vision-v2", 1)
            .await
            .unwrap()
            .is_none()
    );
    let operation: String =
        sqlx::query_scalar("SELECT operation FROM message_index_outbox WHERE message_id = ?")
            .bind(message_id)
            .fetch_one(state.pool())
            .await
            .unwrap();
    assert_eq!(operation, "upsert");

    sqlx::query("DELETE FROM room_memberships WHERE room_id = ? AND user_id = ?")
        .bind(room.id)
        .bind(owner.id)
        .execute(state.pool())
        .await
        .unwrap();
    assert!(read_authorized_image(&state, &execution, &source, limits)
        .await
        .unwrap()
        .is_none());
    assert!(
        load_cached_projection(&state, &execution, &source, "vision-v1", 1)
            .await
            .unwrap()
            .is_none()
    );
}

#[test]
fn visual_projection_is_bound_to_its_source_message_in_context() {
    let message_id = Uuid::new_v4();
    let mut evidence = vec![VisualEvidence {
        source: "A1".into(),
        message_id: message_id.to_string(),
        sender: "vision-owner".into(),
        sent_at: Utc::now().to_rfc3339(),
        nearby_message: "Release plan".into(),
        attachment_id: Uuid::new_v4().to_string(),
        attachment_file_name: "plan.png".into(),
        projection: VisualProjection {
            summary: "Release date: Friday".into(),
            visible_text: vec!["Friday".into()],
            key_facts: vec!["Launch is Friday".into()],
            uncertainties: Vec::new(),
        },
    }];

    let encoded = bounded_visual_context(&mut evidence).unwrap().unwrap();

    assert!(encoded.contains("source_messages"));
    assert!(encoded.contains("nearby_message"));
    assert!(encoded.contains(&message_id.to_string()));
    assert!(encoded.contains("Release date: Friday"));
}

#[tokio::test]
async fn fresh_schema_contains_room_scoped_visual_projections() {
    let state = AppState::new().await.unwrap();

    sqlx::query(
        "SELECT attachment_id, room_id, model, prompt_version, projection, search_text, \
         created_at, updated_at FROM attachment_visual_projections LIMIT 0",
    )
    .execute(state.pool())
    .await
    .unwrap();
}

fn execution(user_id: Uuid, room_id: Uuid) -> AiRunExecution {
    AiRunExecution {
        id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        user_id,
        user_message_id: Uuid::new_v4(),
        assistant_message_id: Uuid::new_v4(),
        room_id: Some(room_id),
        purpose: "conversation".into(),
        source_after_message_id: None,
        source_through_message_id: None,
        source_message_count: None,
        provider: "openai".into(),
        model: "test".into(),
        base_url: String::new(),
        api_key_env: "TEST_KEY".into(),
        admission_id: None,
        thinking_enabled: false,
        question: "what is in the image?".into(),
    }
}

fn source(
    room_id: Uuid,
    message_id: Uuid,
    attachment_id: Uuid,
    size_bytes: i64,
) -> AiCitationSource {
    AiCitationSource {
        label: "A1".into(),
        room_id,
        message_id,
        sender: "vision-owner".into(),
        sent_at: Utc::now(),
        excerpt: "release screenshot".into(),
        score: None,
        score_kind: "attachment".into(),
        attachment: Some(AiCitationAttachment {
            id: attachment_id,
            file_name: "private.png".into(),
            mime_type: "image/png".into(),
            size_bytes,
            download_url: "/private".into(),
            is_sensitive: false,
        }),
    }
}
