use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::models::GraphSnapshot;
use crate::{state::SharedState, user_handlers::bearer_token};

#[utoipa::path(
    get,
    path = "/api/rooms/{id}/knowledge-graph",
    params(("id" = Uuid, Path, description = "Room id")),
    responses(
        (status = 200, description = "Authorized room knowledge graph", body = GraphSnapshot),
        (status = 401, description = "Missing session"),
        (status = 403, description = "Not an active room member"),
        (status = 404, description = "Room not found"),
        (status = 503, description = "Knowledge graph unavailable")
    )
)]
pub async fn room_graph(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<GraphSnapshot>, StatusCode> {
    state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let user = state
        .session_user(bearer_token(&headers)?)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let membership = state
        .membership_identity(room_id, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !membership.is_some_and(|(status, _)| status == "active") {
        return Err(StatusCode::FORBIDDEN);
    }
    let graph = state
        .knowledge_graph()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let mut snapshot = graph.snapshot(room_id).await.map_err(|error| {
        tracing::warn!(%room_id, "load room knowledge graph failed: {error:#}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let source_ids: Vec<_> = snapshot
        .facts
        .iter()
        .flat_map(|fact| fact.episode_ids.iter().copied())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let authorized = state
        .authorized_retrieved_messages(user.id, room_id, &source_ids)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let allowed: HashSet<_> = authorized.into_iter().map(|message| message.id).collect();
    snapshot.facts.retain(|fact| {
        !fact.episode_ids.is_empty() && fact.episode_ids.iter().all(|id| allowed.contains(id))
    });
    let referenced: HashSet<_> = snapshot
        .facts
        .iter()
        .flat_map(|fact| [fact.source_node_id, fact.target_node_id])
        .collect();
    snapshot.nodes.retain(|node| referenced.contains(&node.id));
    for node in &mut snapshot.nodes {
        node.summary.clear();
    }
    Ok(Json(snapshot))
}
