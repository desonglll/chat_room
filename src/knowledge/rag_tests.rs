use chrono::{TimeZone, Utc};

use super::*;

#[test]
fn langchain_documents_preserve_scores_sources_and_exclude_recent_messages() {
    let room_id = Uuid::new_v4();
    let recent_id = Uuid::new_v4();
    let historic_id = Uuid::new_v4();
    let messages = vec![
        message(recent_id, "recent duplicate"),
        message(historic_id, "historic release decision"),
    ];
    let candidates = vec![
        ScoredMessageId {
            id: recent_id,
            score: 0.94,
        },
        ScoredMessageId {
            id: historic_id,
            score: 0.87,
        },
    ];

    let documents = documents_from_messages(
        room_id,
        candidates,
        messages,
        &HashSet::from([recent_id]),
        6,
    );

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].page_content, "historic release decision");
    assert_eq!(documents[0].score, 0.87);
    assert_eq!(documents[0].metadata["source"], "S1");
    assert_eq!(documents[0].metadata["message_id"], historic_id.to_string());
    assert_eq!(documents[0].metadata["room_id"], room_id.to_string());
}

#[test]
fn rag_context_is_structured_for_source_citations() {
    let id = Uuid::new_v4();
    let documents_room_id = Uuid::new_v4();
    let documents = documents_from_messages(
        documents_room_id,
        vec![ScoredMessageId { id, score: 0.8764 }],
        vec![message(id, "The launch date is Friday")],
        &HashSet::new(),
        6,
    );

    let context = render_rag_context(documents).unwrap();

    assert_eq!(context.message_count, 1);
    assert_eq!(context.sources.len(), 1);
    assert_eq!(context.sources[0].label, "S1");
    assert_eq!(context.sources[0].room_id, documents_room_id);
    assert_eq!(context.sources[0].message_id, id);
    assert_eq!(context.sources[0].sender, "Ada");
    assert_eq!(context.sources[0].excerpt, "The launch date is Friday");
    assert_eq!(context.sources[0].score, Some(0.876));
    assert!(context.toon_context.contains("retrieved_evidence"));
    assert!(context.toon_context.contains("S1"));
    assert!(context.toon_context.contains("0.876"));
    assert!(context.toon_context.contains("The launch date is Friday"));
}

#[test]
fn rerank_selection_drops_the_weak_tail_relative_to_the_best_match() {
    let documents = vec![
        Document::new("best").with_score(0.746),
        Document::new("relevant").with_score(0.654),
        Document::new("weak").with_score(0.253),
        Document::new("noise").with_score(0.155),
    ];

    let selected = select_reranked_documents(documents, 6);

    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].page_content, "best");
    assert_eq!(selected[1].page_content, "relevant");
}

#[test]
fn parallel_research_uses_non_overlapping_source_labels() {
    let mut documents = vec![Document::new("one"), Document::new("two")];

    relabel_documents(&mut documents, 100);

    assert_eq!(documents[0].metadata["source"], "S101");
    assert_eq!(documents[1].metadata["source"], "S102");
}

#[test]
fn attachment_evidence_keeps_authorized_preview_metadata() {
    let message_id = Uuid::new_v4();
    let attachment_id = Uuid::new_v4();
    let access_key = Uuid::new_v4();
    let mut attached = message(message_id, "界面截图");
    attached.attachment_id = Some(attachment_id);
    attached.attachment_access_key = Some(access_key);
    attached.attachment_file_name = Some("screen.png".into());
    attached.attachment_mime_type = Some("image/png".into());
    attached.attachment_size_bytes = Some(2048);
    attached.attachment_is_sensitive = Some(true);
    let documents = documents_from_messages(
        Uuid::new_v4(),
        vec![ScoredMessageId {
            id: message_id,
            score: 0.8,
        }],
        vec![attached],
        &HashSet::new(),
        6,
    );

    let source = render_rag_context(documents).unwrap().sources.remove(0);
    let attachment = source.attachment.unwrap();

    assert_eq!(attachment.id, attachment_id);
    assert_eq!(attachment.file_name, "screen.png");
    assert_eq!(attachment.mime_type, "image/png");
    assert_eq!(
        attachment.download_url,
        format!("/api/attachments/{attachment_id}?key={access_key}")
    );
    assert!(attachment.is_sensitive);
}

#[test]
fn rerank_document_combines_message_and_visual_projection_text() {
    let message_id = Uuid::new_v4();
    let mut attached = message(message_id, "季度指标截图");
    attached.attachment_id = Some(Uuid::new_v4());
    attached.attachment_visual_text = Some("Revenue increased to 42 percent".into());

    let documents = documents_from_messages(
        Uuid::new_v4(),
        vec![ScoredMessageId {
            id: message_id,
            score: 0.8,
        }],
        vec![attached],
        &HashSet::new(),
        6,
    );

    assert!(documents[0].page_content.contains("季度指标截图"));
    assert!(documents[0]
        .page_content
        .contains("Revenue increased to 42 percent"));
}

fn message(id: Uuid, content: &str) -> RetrievedMessage {
    RetrievedMessage {
        id,
        sender: "Ada".into(),
        content: content.into(),
        created_at: Utc.with_ymd_and_hms(2026, 8, 25, 10, 0, 0).unwrap(),
        attachment_id: None,
        attachment_access_key: None,
        attachment_file_name: None,
        attachment_mime_type: None,
        attachment_size_bytes: None,
        attachment_is_sensitive: None,
        attachment_visual_text: None,
    }
}
