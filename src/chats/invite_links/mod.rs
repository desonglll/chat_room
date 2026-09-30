//! Invite links (TG-205): a chat's primary link plus any number of additional links, each with
//! an optional expiry, usage limit and approval step, and the join-by-link path.
//!
//! The link token is the capability. It is random (`new_token`), never derived from a row id,
//! and never logged. A join through a link bypasses the chat password and join policy — the
//! administrator who issued the link already decided — but never a ban, and a link whose
//! approval flag is set queues the joiner in the existing join-request queue
//! (`chat_members.status = 'pending'`), where the existing approve / reject action settles it.
//!
//! `usage_count` is consumed by one conditional `UPDATE` inside the join transaction, so the
//! limit holds under any number of concurrent joiners on both adapters (`join.rs`).

pub mod handlers;
pub mod join;
pub mod join_handlers;
pub mod people;
pub mod store;

use std::sync::Arc;

use axum::{
    routing::{get, post, put},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Every invite-link route. Canonical `/api/chats/*` only: the frozen clients behind the
/// `/api/rooms/*` alias predate invite links.
pub fn routes() -> Router<Arc<crate::state::AppState>> {
    Router::new()
        .route(
            "/api/chats/:id/invite-links",
            get(handlers::list_links).post(handlers::create_link),
        )
        .route(
            "/api/chats/:id/invite-links/primary",
            post(handlers::replace_primary),
        )
        .route(
            "/api/chats/:id/invite-links/:link_id",
            put(handlers::edit_link).delete(handlers::delete_link),
        )
        .route(
            "/api/chats/:id/invite-links/:link_id/revoke",
            post(handlers::revoke_link),
        )
        .route(
            "/api/chats/:id/invite-links/:link_id/members",
            get(handlers::link_members),
        )
        .route(
            "/api/chats/:id/invite-links/:link_id/requests",
            get(handlers::link_requests),
        )
        .route("/api/invite-links/:token", get(join_handlers::preview))
        .route("/api/invite-links/:token/join", post(join_handlers::join))
}

/// Telegram's limit on a link's name.
pub const MAX_TITLE_CHARS: usize = 32;
/// Telegram's upper bound on `usage_limit`.
pub const MAX_USAGE_LIMIT: i64 = 99_999;

/// What a link would do if used right now. Computed at read time from the row and the clock,
/// never stored, so an expiry takes effect without any sweeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InviteLinkState {
    Active,
    Expired,
    LimitReached,
    Revoked,
}

impl InviteLinkState {
    pub const fn as_str(self) -> &'static str {
        match self {
            InviteLinkState::Active => "active",
            InviteLinkState::Expired => "expired",
            InviteLinkState::LimitReached => "limit_reached",
            InviteLinkState::Revoked => "revoked",
        }
    }
}

/// One invite link as its managers see it. Only a holder of the link-management right ever
/// receives one — the token is the capability.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct InviteLink {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub token: String,
    pub title: String,
    pub creator_id: Option<Uuid>,
    /// Display name, else username, of the creator; empty once the account is deleted.
    pub creator_name: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub usage_limit: Option<i64>,
    /// Joins this link admitted (approved requests included). Never decremented on leave.
    pub usage_count: i64,
    pub requires_approval: bool,
    pub is_primary: bool,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// Join requests that arrived through this link and still wait for a decision.
    pub pending_count: i64,
    #[sqlx(skip)]
    #[schema(value_type = InviteLinkState)]
    pub state: Option<InviteLinkState>,
}

impl InviteLink {
    /// Fill in `state` for `now`.
    pub fn at(mut self, now: DateTime<Utc>) -> Self {
        self.state = Some(link_state(
            self.revoked_at,
            self.expires_at,
            self.usage_limit,
            self.usage_count,
            now,
        ));
        self
    }
}

pub fn link_state(
    revoked_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    usage_limit: Option<i64>,
    usage_count: i64,
    now: DateTime<Utc>,
) -> InviteLinkState {
    if revoked_at.is_some() {
        InviteLinkState::Revoked
    } else if expires_at.is_some_and(|expires| expires <= now) {
        InviteLinkState::Expired
    } else if usage_limit.is_some_and(|limit| usage_count >= limit) {
        InviteLinkState::LimitReached
    } else {
        InviteLinkState::Active
    }
}

/// `GET /api/chats/:id/invite-links`.
#[derive(Debug, Serialize, ToSchema)]
pub struct InviteLinksView {
    /// Primary first, then live links newest first, then revoked links.
    pub links: Vec<InviteLink>,
    /// Whether the viewer may approve / decline the pending requests (`members.review`).
    pub can_review: bool,
    /// Whether the viewer may edit or revoke links other administrators created.
    pub can_manage_others: bool,
}

/// Create or edit an additional link. Edits replace every field.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct InviteLinkRequest {
    #[serde(default)]
    pub title: String,
    /// RFC 3339; `null` = never expires. Must lie in the future.
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// 1–99 999; `null` = unlimited. Not allowed together with `requires_approval`.
    #[serde(default)]
    pub usage_limit: Option<i64>,
    #[serde(default)]
    pub requires_approval: bool,
}

/// A validated [`InviteLinkRequest`], expiry truncated to whole seconds (SQLite compares the
/// stored RFC 3339 text, which is order-correct for one precision).
#[derive(Debug, Clone, PartialEq)]
pub struct InviteLinkSettings {
    pub title: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub usage_limit: Option<i64>,
    pub requires_approval: bool,
}

impl InviteLinkRequest {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<InviteLinkSettings, &'static str> {
        let title = self.title.trim();
        if title.chars().count() > MAX_TITLE_CHARS || title.chars().any(char::is_control) {
            return Err("title must be at most 32 characters");
        }
        let expires_at = self
            .expires_at
            .map(|expires| DateTime::from_timestamp(expires.timestamp(), 0).unwrap_or(expires));
        if expires_at.is_some_and(|expires| expires <= now) {
            return Err("expires_at must lie in the future");
        }
        if self
            .usage_limit
            .is_some_and(|limit| !(1..=MAX_USAGE_LIMIT).contains(&limit))
        {
            return Err("usage_limit must be between 1 and 99999");
        }
        if self.requires_approval && self.usage_limit.is_some() {
            return Err("a link that requires approval cannot have a usage limit");
        }
        Ok(InviteLinkSettings {
            title: title.to_string(),
            expires_at,
            usage_limit: self.usage_limit,
            requires_approval: self.requires_approval,
        })
    }
}

/// One account on a link's joined list or pending list.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct InviteLinkMember {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
    pub status: String,
    pub requested_at: Option<DateTime<Utc>>,
    pub joined_at: Option<DateTime<Utc>>,
}

/// `GET /api/invite-links/:token` — what a prospective member may see before joining.
/// The chat id is withheld until the viewer is an active member (it is not a capability, but
/// a preview has no use for it).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InvitePreview {
    pub title: String,
    pub description: String,
    pub avatar_emoji: String,
    pub chat_type: super::ChatType,
    pub member_count: i64,
    pub requires_approval: bool,
    /// The viewer's membership status in the chat, if any (`active`, `pending`, `invited`).
    pub membership_status: Option<String>,
    pub chat_id: Option<Uuid>,
}

/// `POST /api/invite-links/:token/join`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InviteJoinResult {
    /// `active` (joined, or already a member) or `pending` (queued for approval).
    pub status: String,
    /// Present when `status` is `active`.
    pub chat_id: Option<Uuid>,
}

/// A fresh link token: 32 bytes from two v4 UUIDs (the OS CSPRNG through `getrandom`),
/// base64url without padding — 43 URL-safe characters, never derived from any id.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Whether a path segment can be a token at all — rejects junk before it reaches SQL.
pub fn plausible_token(token: &str) -> bool {
    (16..=64).contains(&token.len())
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn tokens_are_random_and_url_safe() {
        let first = new_token();
        let second = new_token();
        assert_ne!(first, second);
        assert_eq!(first.len(), 43);
        assert!(plausible_token(&first));
        assert!(!plausible_token("../etc"));
        assert!(!plausible_token("short"));
    }

    #[test]
    fn state_follows_the_clock() {
        let now = Utc::now();
        let past = Some(now - Duration::seconds(1));
        assert_eq!(
            link_state(None, None, None, 0, now),
            InviteLinkState::Active
        );
        assert_eq!(
            link_state(None, past, None, 0, now),
            InviteLinkState::Expired
        );
        assert_eq!(
            link_state(None, None, Some(2), 2, now),
            InviteLinkState::LimitReached
        );
        assert_eq!(
            link_state(Some(now), None, None, 0, now),
            InviteLinkState::Revoked
        );
    }

    #[test]
    fn requests_are_validated() {
        let now = Utc::now();
        let request = |expires_at, usage_limit, requires_approval| InviteLinkRequest {
            title: " 朋友 ".into(),
            expires_at,
            usage_limit,
            requires_approval,
        };
        let ok = request(Some(now + Duration::hours(1)), Some(5), false)
            .validate(now)
            .unwrap();
        assert_eq!(ok.title, "朋友");
        assert_eq!(ok.expires_at.unwrap().timestamp_subsec_nanos(), 0);
        assert!(request(Some(now - Duration::hours(1)), None, false)
            .validate(now)
            .is_err());
        assert!(request(None, Some(0), false).validate(now).is_err());
        assert!(request(None, Some(100_000), false).validate(now).is_err());
        assert!(request(None, Some(3), true).validate(now).is_err());
        let long = InviteLinkRequest {
            title: "x".repeat(33),
            ..InviteLinkRequest::default()
        };
        assert!(long.validate(now).is_err());
    }
}
