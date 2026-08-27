//! Persistent system-administrator roles and one-time registration invitations.

pub mod handlers;
mod models;
mod store;

pub use handlers::{
    create_invite, grant, list, revoke, CreateRegistrationInviteRequest, RegistrationInviteSecret,
};
pub use models::{AdminRoleError, SystemAdminView};

use sha2::{Digest, Sha256};

pub(crate) fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
