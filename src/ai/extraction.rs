use genai::chat::{ChatMessage, ChatRequest};
use serde::{Deserialize, Serialize};

use super::{parse_json_object, AiAssistant};

#[derive(Debug, Serialize)]
pub(crate) struct AiExtractionContextMessage {
    pub label: String,
    pub sent_at: String,
    pub sender: String,
    pub content: String,
    pub attachment: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct AiExtractedCandidate {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub source_labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AiExtractionOutput {
    candidates: Vec<AiExtractedCandidate>,
}

impl AiAssistant {
    pub(crate) async fn extract_decisions_and_tasks(
        &self,
        room_name: &str,
        messages: &[AiExtractionContextMessage],
    ) -> anyhow::Result<Vec<AiExtractedCandidate>> {
        let transcript = serde_json::to_string(messages)?;
        let system_prompt = format!(
            "You extract proposed decisions and actionable tasks from a private chat room named {room_name:?}. Return ONLY one JSON object with this exact shape: {{\"candidates\":[{{\"kind\":\"decision|task\",\"title\":\"short title\",\"detail\":\"concise supporting detail\",\"source_labels\":[\"S1\"]}}]}}. Return at most 20 candidates. Use only source labels present in the input. Include every supporting source label. If a useful candidate is a reasonable inference but no message directly supports it, use an empty source_labels array. Do not invent assignees, due dates, task statuses, message IDs, or database IDs. Write in the conversation's main language."
        );
        let request = ChatRequest::new(vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(transcript),
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
        .map_err(|_| anyhow::anyhow!("AI extraction timed out"))??;
        let text = response
            .first_text()
            .ok_or_else(|| anyhow::anyhow!("AI extraction returned no text"))?;
        parse_candidate_output(text)
    }
}

fn parse_candidate_output(text: &str) -> anyhow::Result<Vec<AiExtractedCandidate>> {
    Ok(parse_json_object::<AiExtractionOutput>(text)?.candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_the_structured_candidate_schema() {
        let parsed = parse_candidate_output(
            "```json\n{\"candidates\":[{\"kind\":\"decision\",\"title\":\"Ship Friday\",\"detail\":\"Approved\",\"source_labels\":[\"S2\"]}]}\n```",
        )
        .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].kind, "decision");
        assert_eq!(parsed[0].source_labels, ["S2"]);
    }

    #[test]
    fn rejects_fields_that_could_bypass_server_owned_task_state() {
        let error = parse_candidate_output(
            r#"{"candidates":[{"kind":"task","title":"Ship","detail":"","source_labels":[],"assignee_id":"someone"}]}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
}
