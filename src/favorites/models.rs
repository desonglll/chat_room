use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::Attachment;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FavoriteItem {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_username: String,
    pub owner_display_name: String,
    pub access: String,
    pub version: i64,
    pub collaborator_count: i64,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub source_message_id: Option<Uuid>,
    pub source_room_id: Option<Uuid>,
    pub source_sender: String,
    pub source_room_name: String,
    pub attachment: Option<Attachment>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FavoriteCollaborator {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFavoriteRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFavoriteRequest {
    pub version: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddFavoriteCollaboratorRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FavoriteMessagesRequest {
    pub message_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ForwardFavoriteRequest {
    pub target_room_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FavoriteForwardResult {
    pub favorite_id: Uuid,
    pub target_room_id: Uuid,
    pub forwarded_message_id: Option<Uuid>,
    pub skipped_reason: Option<String>,
}
