mod client;
pub mod handlers;
pub mod models;
mod retrieval;
mod store;
mod worker;

pub use client::KnowledgeGraph;
pub(crate) use retrieval::retrieve_graph_context;
pub use worker::ensure_worker;

pub(crate) fn connect_or_disable(
    config: &crate::config::KnowledgeGraphConfig,
) -> Option<KnowledgeGraph> {
    match KnowledgeGraph::connect(config) {
        Ok(graph) => graph,
        Err(error) => {
            tracing::warn!("knowledge graph disabled after configuration error: {error:#}");
            None
        }
    }
}
