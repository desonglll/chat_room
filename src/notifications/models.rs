use std::{fmt, str::FromStr};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    FriendRequest,
    RoomJoinRequest,
    Mention,
    Reply,
    AiRunCompleted,
}

impl NotificationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FriendRequest => "friend_request",
            Self::RoomJoinRequest => "room_join_request",
            Self::Mention => "mention",
            Self::Reply => "reply",
            Self::AiRunCompleted => "ai_run_completed",
        }
    }
}

impl FromStr for NotificationKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "friend_request" => Ok(Self::FriendRequest),
            "room_join_request" => Ok(Self::RoomJoinRequest),
            "mention" => Ok(Self::Mention),
            "reply" => Ok(Self::Reply),
            "ai_run_completed" => Ok(Self::AiRunCompleted),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NotificationCursor {
    pub created_at: DateTime<Utc>,
    pub id: String,
}

impl fmt::Display for NotificationCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}|{}",
            self.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            self.id
        )
    }
}

impl FromStr for NotificationCursor {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (created_at, id) = value.rsplit_once('|').ok_or(())?;
        if id.is_empty() || id.chars().count() > 300 {
            return Err(());
        }
        Ok(Self {
            created_at: DateTime::parse_from_rfc3339(created_at)
                .map_err(|_| ())?
                .with_timezone(&Utc),
            id: id.to_owned(),
        })
    }
}

#[derive(Debug)]
pub struct NotificationQuery {
    pub kind: Option<NotificationKind>,
    pub cursor: Option<NotificationCursor>,
    pub limit: i64,
}

#[derive(Debug)]
pub struct NotificationEvent {
    pub recipient_id: Uuid,
    pub kind: NotificationKind,
    pub actor_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub message_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub summary: String,
    pub dedupe_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct NotificationActor {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct NotificationView {
    pub id: String,
    pub kind: NotificationKind,
    pub actor: Option<NotificationActor>,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub message_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub summary: String,
    pub source_available: bool,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationPage {
    pub items: Vec<NotificationView>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, ToSchema)]
pub struct UnreadCount {
    pub unread_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NotificationAccountState {
    pub unread_count: i64,
    pub latest_notification_id: Option<String>,
}
