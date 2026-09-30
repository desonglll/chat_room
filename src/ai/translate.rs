//! TG-410: message translation through the configured AI provider. A translation is a
//! projection (CONTEXT.md): it is returned to the one viewer who asked and never written back
//! over the original message.

use genai::chat::{ChatMessage, ChatRequest};

use super::AiAssistant;

/// Longest text sent for translation (characters); a chat message is at most 4096.
pub const MAX_TRANSLATE_CHARS: usize = 4096;

impl AiAssistant {
    pub(crate) async fn translate(
        &self,
        text: &str,
        target_language: &str,
    ) -> anyhow::Result<String> {
        let system_prompt = format!(
            "Translate the user's message into {target_language:?}. Return ONLY the translation, \
             with no quotes, notes or explanations. Keep emoji, URLs, @mentions and code unchanged. \
             If the message is already in {target_language:?}, return it unchanged."
        );
        let request = ChatRequest::new(vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(text.chars().take(MAX_TRANSLATE_CHARS).collect::<String>()),
        ]);
        let response = tokio::time::timeout(
            self.request_timeout,
            self.client.exec_chat(
                self.model_for(false),
                request,
                self.chat_options(false).as_ref(),
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("AI translation timed out"))??;
        Ok(response
            .first_text()
            .ok_or_else(|| anyhow::anyhow!("AI translation returned no text"))?
            .trim()
            .to_string())
    }
}
