//! The browser-safe projection of the runtime configuration served at
//! `GET /api/config`.
//!
//! Nothing credential-bearing may be added here: this response is readable by
//! any client. AI readiness is reported as a coarse status derived from the
//! stored model options, never as the keys themselves.

use axum::{extract::State, Json};
use serde::Serialize;

use crate::ai::AiRuntimeStatus;
use crate::state::SharedState;

#[derive(Serialize)]
pub struct PublicConfig {
    max_upload_bytes: usize,
    ai_enabled: bool,
    ai_status: AiRuntimeStatus,
    registration_mode: String,
}

pub async fn public_config(State(state): State<SharedState>) -> Json<PublicConfig> {
    let choices = state.ai_model_choices().await.unwrap_or_default();
    let ai_status = if choices.iter().any(|choice| choice.ready) {
        AiRuntimeStatus::Ready
    } else if choices.is_empty() {
        AiRuntimeStatus::Disabled
    } else {
        AiRuntimeStatus::MissingCredentials
    };
    Json(PublicConfig {
        max_upload_bytes: state.max_upload_bytes(),
        ai_enabled: ai_status == AiRuntimeStatus::Ready,
        ai_status,
        registration_mode: state.registration_mode().to_string(),
    })
}
