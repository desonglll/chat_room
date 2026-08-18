//! AI "magic button": summarize recent room activity and suggest replies.
//!
//! Built on the `genai` crate so new providers only need a config change
//! (genai infers the adapter from the model name, e.g. `gpt-*` vs `claude-*`).

use std::time::Duration;

use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::config::AiConfig;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiSuggestions {
    pub summary: String,
    pub suggestions: Vec<String>,
}

/// One line of chat context handed to the model: a resolved display name (never
/// a raw user id) plus the message text.
pub struct AiContextMessage {
    pub sender: String,
    pub content: String,
}

#[derive(Clone)]
pub struct AiAssistant {
    client: Client,
    model: String,
}

impl AiAssistant {
    pub fn new(config: &AiConfig) -> Self {
        let env_name = config.api_key_env.clone();
        let adapter_kind = match config.provider.as_str() {
            "anthropic" => AdapterKind::Anthropic,
            _ => AdapterKind::OpenAI,
        };
        let base_url = config.base_url.clone();

        // A ServiceTargetResolver (rather than a plain AuthResolver) lets us
        // also override the endpoint when `base_url` is set, e.g. for a
        // self-hosted or proxied OpenAI-compatible API.
        let target_resolver = ServiceTargetResolver::from_resolver_fn(
            move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                let ServiceTarget { model, endpoint, .. } = service_target;
                // `api_key_env` may be the *name* of an environment variable
                // (recommended) or, if no such variable is set, the API key
                // itself — so a key can be pasted directly into the TOML
                // config without an extra indirection step.
                let key = std::env::var(&env_name).unwrap_or_else(|_| env_name.clone());
                let auth = AuthData::from_single(key);
                let model = ModelIden::new(adapter_kind, model.model_name);
                let endpoint = match &base_url {
                    Some(url) => Endpoint::from_owned(url.clone()),
                    None => endpoint,
                };
                Ok(ServiceTarget { endpoint, auth, model })
            },
        );
        let client = Client::builder()
            .with_service_target_resolver(target_resolver)
            .build();
        Self {
            client,
            model: config.model.clone(),
        }
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
        let mut transcript = String::new();
        for message in context {
            transcript.push_str(&message.sender);
            transcript.push_str(": ");
            transcript.push_str(&message.content);
            transcript.push('\n');
        }
        if transcript.is_empty() {
            transcript.push_str("(no messages yet)");
        }

        let system_prompt = format!(
            "You are a helpful assistant embedded in the chat room \"{room_name}\". \
             Given the recent conversation transcript, do two things: \
             1) write a one-sentence summary of what's being discussed, in the same \
             language the conversation is mostly written in; \
             2) suggest 3 short, natural next messages the current user might want to send. \
             Respond with ONLY a single JSON object, no markdown fences, no extra text, \
             in exactly this shape: \
             {{\"summary\": \"...\", \"suggestions\": [\"...\", \"...\", \"...\"]}}"
        );
        let chat_req = ChatRequest::new(vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(transcript),
        ]);

        let response = tokio::time::timeout(
            REQUEST_TIMEOUT,
            self.client.exec_chat(self.model.as_str(), chat_req, None),
        )
        .await
        .map_err(|_| anyhow::anyhow!("AI request timed out"))??;

        let text = response
            .first_text()
            .ok_or_else(|| anyhow::anyhow!("AI response had no text content"))?;
        parse_suggestions(text)
    }
}

fn parse_suggestions(text: &str) -> anyhow::Result<AiSuggestions> {
    // Models sometimes wrap JSON in prose or markdown fences despite instructions —
    // extract the outermost {...} block defensively instead of failing outright.
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("AI response did not contain a JSON object"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("AI response did not contain a JSON object"))?;
    if end < start {
        anyhow::bail!("AI response had malformed JSON boundaries");
    }
    let suggestions: AiSuggestions = serde_json::from_str(&text[start..=end])?;
    Ok(suggestions)
}
