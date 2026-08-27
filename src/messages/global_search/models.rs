use std::{fmt, str::FromStr};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchContentType {
    #[default]
    All,
    Text,
    File,
    Image,
    Video,
    Audio,
}

impl SearchContentType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Text => "text",
            Self::File => "file",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }

    pub(crate) fn from_mime_type(mime_type: Option<&str>) -> Self {
        match mime_type.and_then(|value| value.split_once('/').map(|(kind, _)| kind)) {
            None => Self::Text,
            Some("image") => Self::Image,
            Some("video") => Self::Video,
            Some("audio") => Self::Audio,
            Some(_) => Self::File,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchCursor {
    pub created_at: DateTime<Utc>,
    pub message_id: Uuid,
}

impl fmt::Display for SearchCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}|{}",
            self.created_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            self.message_id
        )
    }
}

impl FromStr for SearchCursor {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (created_at, message_id) = value.rsplit_once('|').ok_or(())?;
        Ok(Self {
            created_at: DateTime::parse_from_rfc3339(created_at)
                .map_err(|_| ())?
                .with_timezone(&Utc),
            message_id: Uuid::parse_str(message_id).map_err(|_| ())?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct VisibleMessageSearch {
    pub text: String,
    pub room_id: Option<Uuid>,
    pub sender_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub content_type: SearchContentType,
    pub cursor: Option<SearchCursor>,
    pub limit: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GlobalMessageSearchResult {
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub conversation_kind: String,
    pub conversation_title: String,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub excerpt: String,
    pub content_type: SearchContentType,
    pub attachment_file_name: Option<String>,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GlobalMessageSearchPage {
    pub items: Vec<GlobalMessageSearchResult>,
    pub next_cursor: Option<String>,
}
