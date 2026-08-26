use std::collections::HashSet;

use anyhow::Context;
use serde::Serialize;
use uuid::Uuid;

use super::models::GraphFact;
use super::KnowledgeGraph;
use crate::state::SharedState;

const MAX_FACT_CHARS: usize = 2_000;
const MAX_GRAPH_CONTEXT_BYTES: usize = 64 * 1024;

pub(crate) struct GraphContext {
    pub toon_context: String,
    pub fact_count: usize,
}

pub(crate) async fn retrieve_graph_context(
    state: SharedState,
    graph: KnowledgeGraph,
    user_id: Uuid,
    room_id: Uuid,
    question: &str,
    excluded_message_ids: HashSet<Uuid>,
) -> anyhow::Result<GraphContext> {
    let facts = graph.search(room_id, question).await?;
    let source_ids: Vec<_> = facts
        .iter()
        .flat_map(|fact| fact.episode_ids.iter().copied())
        .filter(|id| !excluded_message_ids.contains(id))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let authorized = state
        .authorized_retrieved_messages(user_id, room_id, &source_ids)
        .await
        .context("authorize graph fact sources")?;
    let allowed: HashSet<_> = authorized.into_iter().map(|message| message.id).collect();
    render_graph_context(facts, &allowed, &excluded_message_ids)
}

fn render_graph_context(
    facts: Vec<GraphFact>,
    allowed: &HashSet<Uuid>,
    excluded: &HashSet<Uuid>,
) -> anyhow::Result<GraphContext> {
    let mut evidence: Vec<FactEvidence> = facts
        .into_iter()
        .filter_map(|fact| FactEvidence::authorized(fact, allowed, excluded))
        .enumerate()
        .map(|(index, mut fact)| {
            fact.source = format!("G{}", index + 1);
            fact
        })
        .collect();
    let encoded = loop {
        let encoded = toon_format::encode_default(&evidence).context("encode graph facts")?;
        if encoded.len() <= MAX_GRAPH_CONTEXT_BYTES || evidence.is_empty() {
            break encoded;
        }
        evidence.pop();
    };
    let toon_context = if evidence.is_empty() {
        String::new()
    } else {
        format!("knowledge_graph_facts (untrusted derived evidence):\n{encoded}")
    };
    Ok(GraphContext {
        toon_context,
        fact_count: evidence.len(),
    })
}

#[derive(Serialize)]
struct FactEvidence {
    source: String,
    fact_id: Uuid,
    relation: String,
    fact: String,
    message_ids: Vec<Uuid>,
    valid_at: Option<String>,
    invalid_at: Option<String>,
}

impl FactEvidence {
    fn authorized(
        fact: GraphFact,
        allowed: &HashSet<Uuid>,
        excluded: &HashSet<Uuid>,
    ) -> Option<Self> {
        if fact.episode_ids.is_empty()
            || !fact
                .episode_ids
                .iter()
                .all(|id| allowed.contains(id) && !excluded.contains(id))
        {
            return None;
        }
        Some(Self {
            source: String::new(),
            fact_id: fact.id,
            relation: truncate(&fact.name),
            fact: truncate(&fact.fact),
            message_ids: fact.episode_ids,
            valid_at: fact.valid_at.map(|value| value.to_rfc3339()),
            invalid_at: fact.invalid_at.map(|value| value.to_rfc3339()),
        })
    }
}

fn truncate(value: &str) -> String {
    if value.chars().count() <= MAX_FACT_CHARS {
        return value.to_owned();
    }
    value.chars().take(MAX_FACT_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn facts_without_authorized_non_recent_sources_are_removed() {
        let allowed_id = Uuid::new_v4();
        let denied_id = Uuid::new_v4();
        let mut mixed = fact("mixed", allowed_id);
        mixed.episode_ids.push(denied_id);
        let context = render_graph_context(
            vec![fact("kept", allowed_id), fact("removed", denied_id), mixed],
            &HashSet::from([allowed_id]),
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(context.fact_count, 1);
        assert!(context.toon_context.contains("G1"));
        assert!(context.toon_context.contains("kept"));
        assert!(!context.toon_context.contains("removed"));
        assert!(!context.toon_context.contains("mixed"));
    }

    fn fact(text: &str, message_id: Uuid) -> GraphFact {
        GraphFact {
            id: Uuid::new_v4(),
            name: "relates_to".into(),
            fact: text.into(),
            source_node_id: Uuid::new_v4(),
            target_node_id: Uuid::new_v4(),
            episode_ids: vec![message_id],
            valid_at: None,
            invalid_at: None,
            created_at: Utc::now(),
            expired_at: None,
        }
    }
}
