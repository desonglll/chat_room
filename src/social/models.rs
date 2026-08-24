use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::UserSummary;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SocialUser {
    pub id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
    pub display_name: String,
    pub signature: String,
    pub remark: String,
    pub relationship: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FriendRequestView {
    pub user: UserSummary,
    pub direction: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FriendRequestPayload {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FriendRequestAction {
    pub action: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFriendRemarkRequest {
    pub remark: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FriendRequestOutcome {
    Created,
    Pending,
    Accepted,
    RateLimited,
}
