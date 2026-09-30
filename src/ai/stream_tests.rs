use std::sync::{Arc, Mutex};

use axum::{
    extract::State, http::header::CONTENT_TYPE, response::IntoResponse, routing::post, Json, Router,
};
use futures_util::StreamExt;

use super::super::AiConfig;
use super::*;

#[test]
fn retrieved_evidence_prompt_requires_source_citations() {
    let messages = conversation_messages(
        Some("retrieved_evidence:\nsource: S1"),
        &[],
        "When is launch?",
        true,
        Some("查找事实"),
    );
    let encoded = serde_json::to_string(&messages).unwrap();

    assert!(encoded.contains("authorized conversation context"));
    assert!(encoded.contains("exact label such as [S1]"));
    assert!(encoded.contains("retrieved_evidence"));
    assert!(encoded.contains("full room history"));
    assert!(encoded.contains("participants, events, chronology"));
    assert!(encoded.contains("查找事实"));
}

#[test]
fn broad_summary_prompt_requires_reviewing_every_visual_projection() {
    let messages = conversation_messages(
        Some(
            "source_messages[1]{source,message_id,attachment_id,projection}:\n\
             A1,message-1,attachment-1,{summary:\"whiteboard\",uncertainties:[\"date unclear\"]}",
        ),
        &[],
        "Summarize everything in the room, including the images.",
        false,
        Some("conversation summary"),
    );
    let encoded = serde_json::to_string(&messages).unwrap();

    assert!(encoded.contains("source_messages"));
    assert!(encoded.contains("review every supplied visual projection"));
    assert!(encoded.contains("preserve its source label and uncertainty"));
    assert!(encoded.contains("A1"));
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
