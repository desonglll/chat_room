//! Wire types of the TG-201 administration contract (`docs/devlog/TG-201.md`, Frozen
//! interface). Kept beside the domain rather than in `src/models.rs` (AGENTS.md).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::chat_type::ChatType;
use super::permissions::PermissionDescriptor;
use super::restrictions::MemberRestriction;

/// `GET /api/chats/:id/permissions` — what the chat allows and what the viewer may do.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChatPermissionsView {
    pub chat_id: Uuid,
    pub chat_type: ChatType,
    pub member_count: i64,
    /// The member-scope keys every ordinary member holds (Telegram's "成员权限").
    pub default_permissions: Vec<String>,
    /// Every registered key the viewer holds right now, restrictions and chat type applied.
    pub my_permissions: Vec<String>,
    /// The viewer's role: `owner`, `admin` or `member`.
    pub my_role: String,
    /// The whole registry with scopes and labels, in display order.
    pub registry: Vec<PermissionDescriptor>,
}

/// `PUT /api/chats/:id/default-permissions`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DefaultPermissionsWrite {
    pub permissions: Vec<String>,
}

/// One member as the roster, admin list and editors show them.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChatMemberEntry {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
    pub nickname: String,
    /// `owner`, `admin` or `member`.
    pub role: String,
    pub status: String,
    pub joined_at: Option<DateTime<Utc>>,
    /// An administrator's title; empty for everyone else.
    pub custom_title: String,
    /// An administrator's rights. Present only for administrators, and only when the viewer
    /// may appoint administrators or is looking at themself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_rights: Option<Vec<String>>,
    /// Restrictions in force. Present only when the viewer may restrict members or is
    /// looking at themself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restrictions: Option<Vec<MemberRestriction>>,
}

/// `GET /api/chats/:id/members/page`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChatMemberPage {
    pub items: Vec<ChatMemberEntry>,
    /// Opaque keyset cursor for the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// `PUT /api/chats/:id/members/:user_id/admin`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AdminAppointment {
    /// Keys from the admin-assignable set; the baseline (sending, own messages) is implied.
    pub permissions: Vec<String>,
    #[serde(default)]
    pub custom_title: String,
}

/// `PUT /api/chats/:id/members/:user_id/restrictions`. An empty list lifts everything.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RestrictionWrite {
    pub denied_permissions: Vec<String>,
    /// `None` = until lifted by hand. Must be in the future.
    pub until: Option<DateTime<Utc>>,
}
