use anyhow::{bail, Result};
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
    pub vision_max_total_images: usize,
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

    /// Reject an `[ai]` section that cannot possibly work. Credential
    /// availability is deliberately *not* checked: an optional AI
    /// misconfiguration must not prevent the chat server from starting, it is
    /// reported at runtime through [`AiConfig::runtime_status`].
    pub(crate) fn validate(&self) -> Result<()> {
        for (name, body) in [
            ("ai.standard_extra_body", &self.standard_extra_body),
            ("ai.reasoning_extra_body", &self.reasoning_extra_body),
        ] {
            if body.as_ref().is_some_and(|value| !value.is_object()) {
                bail!("{name} must be a JSON/TOML object");
            }
        }
        if !self.enabled {
            return Ok(());
        }
        if !matches!(self.provider.as_str(), "openai" | "anthropic") {
            bail!("ai.provider must be 'openai' or 'anthropic'");
        }
        if self.api_key_env.trim().is_empty() {
            bail!("ai.api_key_env is required when ai.enabled is true");
        }
        if self.model.trim().is_empty() {
            bail!("ai.model is required when ai.enabled is true");
        }
        if self.max_context_messages == 0 || self.analysis_context_messages == 0 {
            bail!("AI context message limits must be greater than zero");
        }
        if self.request_timeout_secs == 0 || self.request_timeout_secs > 300 {
            bail!("ai.request_timeout_secs must be between 1 and 300");
        }
        self.validate_vision()?;
        if self.stream_idle_timeout_secs == 0 || self.stream_idle_timeout_secs > 300 {
            bail!("ai.stream_idle_timeout_secs must be between 1 and 300");
        }
        if self.stream_total_timeout_secs < self.stream_idle_timeout_secs
            || self.stream_total_timeout_secs > 1800
        {
            bail!("ai.stream_total_timeout_secs must be between the idle timeout and 1800");
        }
        Ok(())
    }

    fn validate_vision(&self) -> Result<()> {
        if self.vision_model.is_none() {
            return Ok(());
        }
        if self.vision_api_key_env.trim().is_empty() {
            bail!("ai.vision_api_key_env is required when a vision model is configured");
        }
        if self.vision_max_images == 0 || self.vision_max_images > 20 {
            bail!("ai.vision_max_images must be between 1 and 20");
        }
        if self.vision_max_total_images < self.vision_max_images
            || self.vision_max_total_images > 200
        {
            bail!("ai.vision_max_total_images must be between vision_max_images and 200");
        }
        if self.vision_max_image_mib == 0 || self.vision_max_image_mib > 20 {
            bail!("ai.vision_max_image_mib must be between 1 and 20");
        }
        if self.vision_request_timeout_secs == 0 || self.vision_request_timeout_secs > 300 {
            bail!("ai.vision_request_timeout_secs must be between 1 and 300");
        }
        Ok(())
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
            vision_max_total_images: 64,
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
