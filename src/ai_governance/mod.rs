//! Room AI policy, deployment admission control, and privacy-safe usage accounting.

mod admission;
mod governed_stream;
pub mod handlers;
mod models;
mod policy_store;
mod settings_store;
mod usage_store;

pub(crate) use admission::{estimate_tokens, AiAdmission, AiAdmissionRequest};
pub(crate) use governed_stream::GovernedAiStream;
pub use models::{
    AiGovernanceSettings, AiUsageReport, RoomAiPolicy, UpdateAiGovernanceModel,
    UpdateAiGovernanceSettings, UpdateRoomAiPolicy,
};
