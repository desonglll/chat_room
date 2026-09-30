//! Prompt construction and tolerant parsing of model output.
//!
//! Models ignore output-format instructions often enough that extraction has to
//! be defensive; keeping the prompts and the parser in one file means the
//! instruction and the tolerance it needs are edited together.

use genai::chat::{ChatMessage, ChatRequest};
use serde::Deserialize;

use super::context::AiContextMessage;
use super::AiSuggestions;

/// The planning agent's contract: decide how much context to gather before
/// answering. Kept next to its prompt because the field set and the prompt text
/// have to change together.
pub(super) const PLAN_ROOM_TASK_PROMPT: &str = "You are the planning agent for a private chat analysis assistant. Decide how the server should gather context before answering. Return ONLY one JSON object with: intent (overview, todos, decisions, search, or general), context_scope (recent or full), semantic_search (boolean), and research_questions (zero to three concise search queries). Use full only when the user asks for exhaustive room-wide analysis. Use semantic search for facts or topics that may be outside recent context. Split genuinely multi-topic research into independent research_questions; otherwise return an empty array.";

pub(super) fn suggestion_request(
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

pub(super) fn parse_suggestions(text: &str) -> anyhow::Result<AiSuggestions> {
    parse_json_object(text)
}

/// Extract the outermost `{...}` block and deserialize it. Models sometimes wrap
/// JSON in prose or markdown fences despite instructions, so failing on the raw
/// text would reject otherwise usable answers.
pub(super) fn parse_json_object<T: for<'de> Deserialize<'de>>(text: &str) -> anyhow::Result<T> {
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
