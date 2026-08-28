//! AI "magic button": summarize recent room activity and suggest replies.
//!
//! Built on the `genai` crate so new providers only need a config change
//! (genai infers the adapter from the model name, e.g. `gpt-*` vs `claude-*`).

mod config;
pub(crate) mod extraction;
pub mod model_handlers;
pub mod model_options;
mod stream;
mod vision;

pub use config::{AiConfig, AiRuntimeStatus};
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
pub use model_options::{AiModelChoice, AiModelOptionView, SaveAiModelOption};
use serde::{Deserialize, Serialize};
pub use stream::{AiStreamItem, AiTextStream};
use toon_format::encode_default;
use utoipa::ToSchema;
pub(crate) use vision::{VisionImage, VisionLimits};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiSuggestions {
    pub summary: String,
    pub suggestions: Vec<String>,
}

/// One line of chat context handed to the model: a resolved display name (never
/// a raw user id) plus the message text.
#[derive(Debug, Clone, Serialize)]
pub struct AiContextMessage {
    pub message_id: String,
    pub sent_at: String,
    pub sender: String,
    pub content: String,
    pub source: String,
    pub attachment: String,
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

#[derive(Serialize)]
struct ToonConversation<'a> {
    room: &'a str,
    messages: &'a [AiContextMessage],
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

    pub(crate) async fn describe_image(
        &self,
        question: &str,
        source_label: &str,
        nearby_message: &str,
        image: VisionImage,
    ) -> anyhow::Result<String> {
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
        let chat_req = suggestion_request(room_name, context, false);

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
        parse_suggestions(text)
    }

    pub(crate) async fn plan_room_task(
        &self,
        question: &str,
    ) -> anyhow::Result<AiTaskPlanDecision> {
        let system_prompt =
            "You are the planning agent for a private chat analysis assistant. Decide how the server should gather context before answering. Return ONLY one JSON object with: intent (overview, todos, decisions, search, or general), context_scope (recent or full), semantic_search (boolean), and research_questions (zero to three concise search queries). Use full only when the user asks for exhaustive room-wide analysis. Use semantic search for facts or topics that may be outside recent context. Split genuinely multi-topic research into independent research_questions; otherwise return an empty array.";
        let request = ChatRequest::new(vec![
            ChatMessage::system(system_prompt),
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
        parse_json_object(text)
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

fn suggestion_request(
    room_name: &str,
    context: &[AiContextMessage],
    streaming: bool,
) -> ChatRequest {
    let mut transcript = String::new();
    for message in context {
        transcript.push_str(&message.sender);
        transcript.push_str(": ");
        transcript.push_str(&message.content);
        if !message.attachment.is_empty() {
            transcript.push_str(" [attachment: ");
            transcript.push_str(&message.attachment);
            transcript.push(']');
        }
        transcript.push('\n');
    }
    if transcript.is_empty() {
        transcript.push_str("(no messages yet)");
    }
    let output_rules = if streaming {
        "Respond with ONLY four newline-delimited JSON objects (NDJSON), one per line and no markdown fences. Output the best suggestion first, then two more suggestions, then the summary: {\"type\":\"suggestion\",\"content\":\"...\"} (three lines) and {\"type\":\"summary\",\"content\":\"...\"} (one line)."
    } else {
        "Respond with ONLY one JSON object, no markdown fences or extra text, exactly: {\"summary\":\"...\",\"suggestions\":[\"...\",\"...\",\"...\"]}."
    };
    let system_prompt = format!(
        "You are a helpful assistant embedded in the chat room \"{room_name}\". Write in the conversation's main language. Suggest 3 short, natural next messages the current user might send and a one-sentence summary. {output_rules}"
    );
    ChatRequest::new(vec![
        ChatMessage::system(system_prompt),
        ChatMessage::user(transcript),
    ])
}

pub fn conversation_context_to_toon(
    room_name: &str,
    context: &[AiContextMessage],
) -> anyhow::Result<String> {
    encode_default(&ToonConversation {
        room: room_name,
        messages: context,
    })
    .map_err(|error| anyhow::anyhow!("encode TOON context: {error}"))
}

pub fn bounded_conversation_context_to_toon(
    room_name: &str,
    context: &mut Vec<AiContextMessage>,
    max_bytes: usize,
) -> anyhow::Result<String> {
    loop {
        let encoded = conversation_context_to_toon(room_name, context)?;
        if encoded.len() <= max_bytes || context.len() <= 1 {
            return Ok(encoded);
        }
        context.remove(0);
    }
}

fn parse_suggestions(text: &str) -> anyhow::Result<AiSuggestions> {
    // Models sometimes wrap JSON in prose or markdown fences despite instructions —
    // extract the outermost {...} block defensively instead of failing outright.
    parse_json_object(text)
}

fn parse_json_object<T: for<'de> Deserialize<'de>>(text: &str) -> anyhow::Result<T> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("AI response did not contain a JSON object"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("AI response did not contain a JSON object"))?;
    if end < start {
        anyhow::bail!("AI response had malformed JSON boundaries");
    }
    serde_json::from_str(&text[start..=end]).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::State, http::header::CONTENT_TYPE, response::IntoResponse, routing::post, Json,
        Router,
    };
    use futures_util::StreamExt;

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

    #[test]
    fn conversation_context_uses_a_uniform_toon_message_table() {
        let encoded = conversation_context_to_toon(
            "Project",
            &[
                AiContextMessage {
                    message_id: "message-1".into(),
                    sent_at: "2026-08-25T10:00:00Z".into(),
                    sender: "Ada".into(),
                    content: "ship it".into(),
                    source: String::new(),
                    attachment: String::new(),
                },
                AiContextMessage {
                    message_id: "message-2".into(),
                    sent_at: "2026-08-25T10:01:00Z".into(),
                    sender: "Lin".into(),
                    content: "review first".into(),
                    source: "A1".into(),
                    attachment: "plan.pdf".into(),
                },
            ],
        )
        .unwrap();
        assert!(
            encoded.contains("messages[2]{message_id,sent_at,sender,content,source,attachment}:")
        );
        for value in [
            "message-1",
            "Ada",
            "ship it",
            "message-2",
            "Lin",
            "review first",
            "A1",
            "plan.pdf",
        ] {
            assert!(encoded.contains(value), "missing TOON value: {value}");
        }
    }

    #[test]
    fn context_byte_limit_discards_oldest_messages_first() {
        let mut context = (0..8)
            .map(|index| AiContextMessage {
                message_id: format!("message-{index}"),
                sent_at: format!("2026-08-25T10:0{index}:00Z"),
                sender: "Ada".into(),
                content: format!("message-{index}-{}", "x".repeat(80)),
                source: String::new(),
                attachment: String::new(),
            })
            .collect();
        let encoded = bounded_conversation_context_to_toon("Project", &mut context, 420).unwrap();
        assert!(encoded.len() <= 420);
        assert!(context.len() < 8);
        assert!(!encoded.contains("message-0"));
        assert!(encoded.contains("message-7"));
    }

    #[tokio::test]
    async fn conversation_answer_stream_preserves_chunks_and_v1_base_path() {
        async fn openai_stream(
            State(requests): State<Arc<Mutex<Vec<serde_json::Value>>>>,
            Json(payload): Json<serde_json::Value>,
        ) -> impl IntoResponse {
            requests.lock().unwrap().push(payload);
            let body = concat!(
                "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"分析\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"你\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"好\"},\"finish_reason\":null}]}\n\n",
                "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n"
            );
            ([(CONTENT_TYPE, "text/event-stream")], body)
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = requests.clone();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/chat/completions", post(openai_stream))
                    .with_state(server_requests),
            )
            .await
            .unwrap()
        });
        let assistant = AiAssistant::new(
            &AiConfig {
                provider: "openai".into(),
                model: "gpt-test".into(),
                base_url: Some(format!("http://{address}/v1")),
                standard_extra_body: Some(serde_json::json!({
                    "enable_thinking": false
                })),
                request_timeout_secs: 5,
                ..AiConfig::default()
            },
            "test-key".into(),
        );

        let mut stream = assistant
            .answer_stream(
                Some("room: test"),
                &[],
                "总结",
                false,
                false,
                Some("会话总结"),
            )
            .await
            .unwrap();
        let mut chunks = Vec::new();
        let mut reasoning_seen = false;
        while let Some(item) = stream.next().await {
            match item.unwrap() {
                AiStreamItem::Reasoning => reasoning_seen = true,
                AiStreamItem::Content(chunk) => chunks.push(chunk),
            }
        }

        assert!(reasoning_seen);
        assert_eq!(chunks, ["你", "好"]);
        assert_eq!(requests.lock().unwrap()[0]["enable_thinking"], false);
        server.abort();
    }
}
