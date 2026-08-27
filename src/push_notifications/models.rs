use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SavePushSubscriptionRequest {
    pub endpoint: String,
    pub keys: PushSubscriptionKeys,
    #[serde(default)]
    pub show_details: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PushSubscriptionKeys {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeletePushSubscriptionRequest {
    pub endpoint: String,
}

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct PushSubscriptionView {
    pub id: String,
    pub show_details: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(crate) struct ClaimedPushJob {
    pub id: String,
    pub notification_id: String,
    pub subscription_id: String,
    pub recipient_id: Uuid,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub show_details: bool,
    pub attempts: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct PushPayload {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub url: String,
    pub tag: String,
}
