//! Forwarding keeps albums together (Telegram): when two or more items of one source album
//! are forwarded to the same target in one request, their copies share a fresh `grouped_id`
//! there. A single forwarded item arrives as an ordinary message.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

/// The order to forward in and which requested messages travel as (part of) an album.
#[derive(Debug, Default)]
pub struct ForwardPlan {
    /// The requested ids, oldest source message first (unknown ids last, in request order),
    /// so album items keep their order in the target.
    pub order: Vec<Uuid>,
    source_groups: HashMap<Uuid, Uuid>,
    target_groups: HashMap<(Uuid, Uuid), Uuid>,
}

impl ForwardPlan {
    /// The `grouped_id` the copy of `message_id` gets in `target_room_id`, if it travels with
    /// at least one other item of its album. Stable for the lifetime of the plan.
    pub fn target_group(&mut self, message_id: Uuid, target_room_id: Uuid) -> Option<Uuid> {
        let source = *self.source_groups.get(&message_id)?;
        Some(
            *self
                .target_groups
                .entry((source, target_room_id))
                .or_insert_with(Uuid::new_v4),
        )
    }
}

impl AppState {
    /// Build the plan for one forward request. Reads only ids, times and album membership;
    /// authorization stays with the per-message forward.
    pub async fn plan_forward(&self, message_ids: &[Uuid]) -> Result<ForwardPlan, sqlx::Error> {
        let mut known: Vec<(DateTime<Utc>, Uuid, Option<Uuid>)> = Vec::new();
        for &id in message_ids {
            let row: Option<(DateTime<Utc>, Option<Uuid>)> = with_pool!(self, |pool| {
                sqlx::query_as("SELECT created_at, grouped_id FROM messages WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
            })?;
            if let Some((created_at, grouped_id)) = row {
                if !known.iter().any(|(_, seen, _)| *seen == id) {
                    known.push((created_at, id, grouped_id));
                }
            }
        }
        known.sort_by_key(|(created_at, id, _)| (*created_at, *id));
        let mut order: Vec<Uuid> = known.iter().map(|(_, id, _)| *id).collect();
        let unknown: Vec<Uuid> = message_ids
            .iter()
            .filter(|id| !order.contains(id))
            .copied()
            .collect();
        order.extend(unknown);

        let mut members: HashMap<Uuid, usize> = HashMap::new();
        for (_, _, group) in &known {
            if let Some(group) = group {
                *members.entry(*group).or_default() += 1;
            }
        }
        let source_groups = known
            .into_iter()
            .filter_map(|(_, id, group)| {
                let group = group?;
                (members.get(&group).copied().unwrap_or(0) >= 2).then_some((id, group))
            })
            .collect();
        Ok(ForwardPlan {
            order,
            source_groups,
            target_groups: HashMap::new(),
        })
    }
}
