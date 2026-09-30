//! AI "magic button": summarize recent chat activity and suggest replies.
//!
//! Built on the `genai` crate so new providers only need a config change
//! (genai infers the adapter from the model name, e.g. `gpt-*` vs `claude-*`).
//!
//! This file owns provider wiring and the request/response types crossing the
//! module boundary. The transcript handed to a model lives in [`context`], the
//! prompts and output parsing in [`prompt`], streaming in [`stream`].

mod config;
mod context;
pub(crate) mod extraction;
pub mod model_handlers;
pub mod model_options;
mod prompt;
mod stream;
mod translate;
mod vision;

pub use config::{AiConfig, AiRuntimeStatus};
pub use context::{
    bounded_conversation_context_to_toon, conversation_context_to_toon, AiContextMessage,
};
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
pub use model_options::{AiModelChoice, AiModelOptionView, SaveAiModelOption};
use serde::{Deserialize, Serialize};
pub use stream::{AiStreamItem, AiTextStream};
use utoipa::ToSchema;
pub(crate) use vision::{VisionImage, VisionLimits, VisualProjection};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiSuggestions {
    pub summary: String,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct AiConversationTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AiTaskPlanDecision {
    pub intent: String,
    pub context_scope: String,
    pub semantic_search: bool,
    #[serde(default)]
    pub research_questions: Vec<String>,
}

#[derive(Clone)]
pub struct AiAssistant {
    pub(super) client: Client,
    pub(super) model: String,
    pub(super) fast_model: Option<String>,
    pub(super) request_timeout: std::time::Duration,
    pub(super) stream_idle_timeout: std::time::Duration,
    pub(super) stream_total_timeout: std::time::Duration,
    standard_extra_body: Option<serde_json::Value>,
    reasoning_extra_body: Option<serde_json::Value>,
    vision: Option<vision::VisionAssistant>,
}

impl AiAssistant {
    pub fn new(config: &AiConfig, api_key: String) -> Self {
        let adapter_kind = match config.provider.as_str() {
            "anthropic" => AdapterKind::Anthropic,
            _ => AdapterKind::OpenAI,
        };
        let base_url = config
            .base_url
            .as_deref()
            .map(|url| format!("{}/", url.trim_end_matches('/')));
        // A ServiceTargetResolver (rather than a plain AuthResolver) lets us
        // also override the endpoint when `base_url` is set, e.g. for a
        // self-hosted or proxied OpenAI-compatible API.
        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                let ServiceTarget {
                    model, endpoint, ..
                } = service_target;
                let auth = AuthData::from_single(api_key.clone());
                let model = ModelIden::new(adapter_kind, model.model_name);
                let endpoint = match &base_url {
                    Some(url) => Endpoint::from_owned(url.clone()),
                    None => endpoint,
                };
                Ok(ServiceTarget {
                    endpoint,
                    auth,
                    model,
                })
            },
        );
        let client = Client::builder()
            .with_service_target_resolver(target_resolver)
            .build();
        Self {
            client,
            model: config.model.clone(),
            fast_model: config
                .fast_model
                .as_ref()
                .map(|model| model.trim().to_owned())
                .filter(|model| !model.is_empty()),
            request_timeout: std::time::Duration::from_secs(config.request_timeout_secs),
            stream_idle_timeout: std::time::Duration::from_secs(config.stream_idle_timeout_secs),
            stream_total_timeout: std::time::Duration::from_secs(config.stream_total_timeout_secs),
            standard_extra_body: config.standard_extra_body.clone(),
            reasoning_extra_body: config.reasoning_extra_body.clone(),
            vision: vision::VisionAssistant::from_config(config),
        }
    }

    pub(crate) fn vision_limits(&self) -> Option<VisionLimits> {
        self.vision.as_ref().map(vision::VisionAssistant::limits)
    }

    pub(crate) fn vision_identity(&self) -> Option<(&str, i64)> {
        self.vision.as_ref().map(vision::VisionAssistant::identity)
    }

    pub(crate) async fn describe_image(
        &self,
        question: &str,
        source_label: &str,
        nearby_message: &str,
        image: VisionImage,
    ) -> anyhow::Result<VisualProjection> {
        self.vision
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("vision model is not configured"))?
            .describe_image(question, source_label, nearby_message, image)
            .await
    }

    /// Summarize `context` (oldest first) and propose a few next messages the
    /// caller might send. Errors are the caller's cue to show a generic
    /// "AI assistant unavailable" message — never leak provider error text
    /// (which can include request details) to the client.
    pub async fn suggest(
        &self,
        room_name: &str,
        context: &[AiContextMessage],
    ) -> anyhow::Result<AiSuggestions> {
        let chat_req = prompt::suggestion_request(room_name, context, false);

        let options = self.chat_options(false);
        let response = tokio::time::timeout(
            self.request_timeout,
            self.client
                .exec_chat(self.model_for(false), chat_req, options.as_ref()),
        )
        .await
        .map_err(|_| anyhow::anyhow!("AI request timed out"))??;

        let text = response
            .first_text()
            .ok_or_else(|| anyhow::anyhow!("AI response had no text content"))?;
        prompt::parse_suggestions(text)
    }

    pub(crate) async fn plan_chat_task(
        &self,
        question: &str,
    ) -> anyhow::Result<AiTaskPlanDecision> {
        let request = ChatRequest::new(vec![
            ChatMessage::system(prompt::PLAN_CHAT_TASK_PROMPT),
            ChatMessage::user(question),
        ]);
        let timeout = self.request_timeout.min(std::time::Duration::from_secs(8));
        let response = tokio::time::timeout(
            timeout,
            self.client.exec_chat(
                self.model_for(false),
                request,
                self.chat_options(false).as_ref(),
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("planning agent timed out"))??;
        let text = response
            .first_text()
            .ok_or_else(|| anyhow::anyhow!("planning agent returned no text"))?;
        prompt::parse_json_object(text)
    }

    pub(super) fn model_for(&self, thinking_enabled: bool) -> &str {
        if thinking_enabled {
            &self.model
        } else {
            self.fast_model.as_deref().unwrap_or(&self.model)
        }
    }

    pub(super) fn chat_options(&self, thinking_enabled: bool) -> Option<ChatOptions> {
        let extra_body = if thinking_enabled {
            self.reasoning_extra_body.as_ref()
        } else {
            self.standard_extra_body.as_ref()
        }?;
        Some(ChatOptions::default().with_extra_body(extra_body.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_uses_the_configured_request_timeout() {
        let config = AiConfig {
            request_timeout_secs: 42,
            ..AiConfig::default()
        };
        let assistant = AiAssistant::new(&config, "test-key".into());
        assert_eq!(
            assistant.request_timeout,
            std::time::Duration::from_secs(42)
        );
    }
}
