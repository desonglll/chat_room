use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Provider configuration for suggestions and conversation analysis.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub enabled: bool,
    pub provider: String,
    pub api_key_env: String,
    pub model: String,
    pub fast_model: Option<String>,
    pub base_url: Option<String>,
    pub standard_extra_body: Option<serde_json::Value>,
    pub reasoning_extra_body: Option<serde_json::Value>,
    pub vision_model: Option<String>,
    pub vision_base_url: Option<String>,
    pub vision_api_key_env: String,
    pub vision_max_images: usize,
    pub vision_max_image_mib: u64,
    pub vision_request_timeout_secs: u64,
    pub max_context_messages: usize,
    pub analysis_context_messages: usize,
    pub request_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub stream_total_timeout_secs: u64,
    pub suggest_cooldown_secs: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AiRuntimeStatus {
    Disabled,
    MissingCredentials,
    Ready,
}

impl AiConfig {
    pub fn runtime_status(&self) -> AiRuntimeStatus {
        self.runtime_status_with(|name| std::env::var(name).ok())
    }

    pub(crate) fn runtime_status_with(
        &self,
        lookup: impl FnOnce(&str) -> Option<String>,
    ) -> AiRuntimeStatus {
        if !self.enabled {
            return AiRuntimeStatus::Disabled;
        }
        match lookup(self.api_key_env.trim()) {
            Some(value) if !value.trim().is_empty() => AiRuntimeStatus::Ready,
            _ => AiRuntimeStatus::MissingCredentials,
        }
    }

    pub(crate) fn resolved_api_key(&self) -> Option<String> {
        (self.runtime_status() == AiRuntimeStatus::Ready)
            .then(|| std::env::var(self.api_key_env.trim()).ok())
            .flatten()
    }

    pub(crate) fn resolved_vision_api_key(&self) -> Option<String> {
        self.vision_model
            .as_ref()
            .filter(|model| !model.trim().is_empty())
            .and_then(|_| std::env::var(self.vision_api_key_env.trim()).ok())
            .filter(|key| !key.trim().is_empty())
    }

    pub(crate) fn vision_max_image_bytes(&self) -> u64 {
        self.vision_max_image_mib.saturating_mul(1024 * 1024)
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "openai".into(),
            api_key_env: String::new(),
            model: String::new(),
            fast_model: None,
            base_url: None,
            standard_extra_body: None,
            reasoning_extra_body: None,
            vision_model: None,
            vision_base_url: None,
            vision_api_key_env: "CHAT_ROOM_AI_API_KEY".into(),
            vision_max_images: 8,
            vision_max_image_mib: 8,
            vision_request_timeout_secs: 60,
            max_context_messages: 30,
            analysis_context_messages: 5_000,
            request_timeout_secs: 60,
            stream_idle_timeout_secs: 30,
            stream_total_timeout_secs: 300,
            suggest_cooldown_secs: 10,
        }
    }
}
