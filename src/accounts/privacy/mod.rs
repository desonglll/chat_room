//! TG-505: Telegram's privacy matrix — who may see or do what with an account.
//!
//! Five dimensions ([`PrivacyKey`]), each a tier ([`PrivacyTier`]: everybody / contacts /
//! nobody) plus per-account allow and deny exceptions. Telegram's phone-number dimension is
//! absent on purpose: accounts in this product have no phone numbers.
//!
//! The rule itself is the pure [`decide`]; everything else in this module loads its inputs
//! (`store`), applies it on a read path (`presence`, `guards`), or exposes it over HTTP
//! (`handlers`). The decision order, first match wins:
//!
//! 1. the owner always sees their own data;
//! 2. an owner who blocked the viewer denies every dimension;
//! 3. an exception about the viewer (deny or allow) beats the tier;
//! 4. the tier: everybody → allow, contacts → accepted friendship, nobody → deny.
//!
//! "Contacts" are accepted friendships (`friendships.status = 'accepted'`), the only
//! contact relation this product has.

mod contact_presence;
mod guards;
pub mod handlers;
mod last_seen;
mod presence;
mod store;

use axum::{
    routing::{get, put},
    Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::state::SharedState;

pub use guards::HIDDEN_FORWARD_LABEL;
pub use last_seen::{obscured_status, LastSeenRecord};
pub(crate) use presence::PresenceFilter;
pub use store::{PrivacyRuleView, PrivacyRuleWrite, PrivacySettings, MAX_PRIVACY_EXCEPTIONS};

/// One privacy dimension. The wire and database spelling is the snake_case name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyKey {
    /// Exact last-seen and online status; others see an obscured tier instead.
    LastSeen,
    /// The uploaded avatar image (`GET /api/users/:id/avatar`).
    ProfilePhoto,
    /// Whether a forward of the owner's message names the owner.
    Forwards,
    /// Who may invite the owner into a group.
    GroupInvites,
    /// Who may send the owner voice messages in a private chat.
    VoiceMessages,
}

impl PrivacyKey {
    pub const ALL: [PrivacyKey; 5] = [
        PrivacyKey::LastSeen,
        PrivacyKey::ProfilePhoto,
        PrivacyKey::Forwards,
        PrivacyKey::GroupInvites,
        PrivacyKey::VoiceMessages,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PrivacyKey::LastSeen => "last_seen",
            PrivacyKey::ProfilePhoto => "profile_photo",
            PrivacyKey::Forwards => "forwards",
            PrivacyKey::GroupInvites => "group_invites",
            PrivacyKey::VoiceMessages => "voice_messages",
        }
    }
}

/// The base audience of a dimension. Absent rule rows mean [`PrivacyTier::Everybody`],
/// Telegram's default for every dimension this product has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyTier {
    #[default]
    Everybody,
    Contacts,
    Nobody,
}

impl PrivacyTier {
    pub fn as_str(self) -> &'static str {
        match self {
            PrivacyTier::Everybody => "everybody",
            PrivacyTier::Contacts => "contacts",
            PrivacyTier::Nobody => "nobody",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "contacts" => PrivacyTier::Contacts,
            "nobody" => PrivacyTier::Nobody,
            _ => PrivacyTier::Everybody,
        }
    }
}

/// A per-account exception to a tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionEffect {
    Allow,
    Deny,
}

impl ExceptionEffect {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ExceptionEffect::Allow => "allow",
            ExceptionEffect::Deny => "deny",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "allow" => Some(ExceptionEffect::Allow),
            "deny" => Some(ExceptionEffect::Deny),
            _ => None,
        }
    }
}

/// Everything [`decide`] needs about one (owner, viewer) pair for one dimension.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrivacyFacts {
    pub is_self: bool,
    pub owner_blocked_viewer: bool,
    pub tier: PrivacyTier,
    pub exception: Option<ExceptionEffect>,
    pub are_contacts: bool,
}

/// The privacy rule. See the module documentation for the order and why.
pub fn decide(facts: PrivacyFacts) -> bool {
    if facts.is_self {
        return true;
    }
    if facts.owner_blocked_viewer {
        return false;
    }
    match facts.exception {
        Some(ExceptionEffect::Deny) => false,
        Some(ExceptionEffect::Allow) => true,
        None => match facts.tier {
            PrivacyTier::Everybody => true,
            PrivacyTier::Contacts => facts.are_contacts,
            PrivacyTier::Nobody => false,
        },
    }
}

pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/users/me/privacy", get(handlers::get_privacy))
        .route(
            "/api/users/me/privacy/:key",
            put(handlers::put_privacy_rule),
        )
}
