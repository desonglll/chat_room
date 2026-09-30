//! The Chat domain: the authorization and knowledge-isolation boundary.
//!
//! `docs/tg/decisions.md` D-003 renamed this module from `rooms` in M0. The database column
//! that references a chat is still `room_id` everywhere, deliberately — see
//! `docs/devlog/TG-004.md` "Frozen interface".

pub mod access;
pub mod admin_handlers;
pub mod admin_models;
pub mod admin_rights;
pub mod authorization;
pub mod bans;
pub mod capabilities;
pub mod channel_handlers;
pub(crate) mod channel_posts;
pub mod channel_views;
pub mod channels;
pub(crate) mod chat_projection;
pub mod chat_type;
pub mod compat;
pub mod default_permissions;
pub mod drafts;
pub mod governance_handlers;
pub mod handlers;
pub mod invite_links;
pub(crate) mod lifecycle;
pub mod lifecycle_handlers;
pub mod member_page;
pub mod membership_handlers;
pub mod membership_mutations;
pub mod message_history;
pub mod message_moderation;
pub mod models;
pub mod participants;
pub mod permissions;
pub mod private_chat_handlers;
pub mod private_chats;
pub mod provisioning;
pub mod public_handles;
pub mod query_handlers;
pub mod restrictions;
pub mod roster_handlers;
pub mod routes;
pub mod slow_mode;
pub mod supergroup_upgrade;
pub mod topics;

pub use authorization::ChatAuthorization;
pub use capabilities::{CapabilityError, CapabilityOutcome, ChatCapabilityChange};
pub use chat_type::ChatType;
pub use compat::ApiDialect;
pub use supergroup_upgrade::{
    supergroup_upgrade_trigger, ChatCapabilityRequest, SupergroupUpgradeTrigger,
};
pub use topics::TopicError;
