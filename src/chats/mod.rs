//! The Chat domain: the authorization and knowledge-isolation boundary.
//!
//! `docs/tg/decisions.md` D-003 renamed this module from `rooms` in M0. The database column
//! that references a chat is still `room_id` everywhere, deliberately — see
//! `docs/devlog/TG-004.md` "Frozen interface".

pub mod access;
pub mod authorization;
pub mod bans;
pub mod chat_type;
pub mod compat;
pub mod drafts;
pub mod governance_handlers;
pub mod handlers;
pub(crate) mod lifecycle;
pub mod lifecycle_handlers;
pub mod membership_handlers;
pub mod membership_mutations;
pub mod message_history;
pub mod models;
pub mod participants;
pub mod private_chat_handlers;
pub mod private_chats;
pub mod provisioning;
pub mod query_handlers;
pub mod routes;
pub mod supergroup_upgrade;

pub use authorization::ChatAuthorization;
pub use chat_type::ChatType;
pub use compat::ApiDialect;
pub use supergroup_upgrade::{
    supergroup_upgrade_trigger, ChatCapabilityRequest, SupergroupUpgradeTrigger,
};
