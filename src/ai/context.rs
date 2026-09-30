//! The chat transcript handed to a model, and its TOON encoding.
//!
//! Owns the context budget: [`bounded_conversation_context_to_toon`] is the only
//! place that decides which messages are dropped when the encoded context does
//! not fit.

use serde::Serialize;

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

#[derive(Serialize)]
struct ToonConversation<'a> {
    room: &'a str,
    messages: &'a [AiContextMessage],
}

pub fn conversation_context_to_toon(
    room_name: &str,
    context: &[AiContextMessage],
) -> anyhow::Result<String> {
    toon_format::encode_default(&ToonConversation {
        room: room_name,
        messages: context,
    })
    .map_err(|error| anyhow::anyhow!("encode TOON context: {error}"))
}

/// Encode `context` (oldest first), dropping the oldest messages until the
/// encoded form fits in `max_bytes`. The newest message is always kept, even if
/// it alone exceeds the budget — an over-budget answer beats an empty one.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
