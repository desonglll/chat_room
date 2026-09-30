//! Reading poll snapshots: aggregation and per-viewer projection.
//!
//! Counts are always computed from `poll_votes` (never stored), so a snapshot can never
//! disagree with the votes it summarises. One call loads any number of polls in four queries,
//! which is what a history page needs.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{PollOption, PollState, StoredMessage};
use crate::state::{with_pool, AppState};

/// `(message_id, question, public_voters, multiple_choice, quiz, correct_option, explanation,
/// revision, closed_at)` as selected from `polls`.
type PollRow = (
    Uuid,
    String,
    bool,
    bool,
    bool,
    Option<i32>,
    Option<String>,
    i64,
    Option<DateTime<Utc>>,
);

/// Per-option counts `(message_id, position, text, voters)`, per-poll distinct voters, and
/// the viewer's own `(message_id, position)` rows.
type AggregateRows = (
    Vec<(Uuid, i32, String, i64)>,
    Vec<(Uuid, i64)>,
    Vec<(Uuid, i32)>,
);

/// `$1, $2, …, $n` starting after `offset` already-bound parameters.
pub(super) fn placeholders(offset: usize, count: usize) -> String {
    (offset + 1..=offset + count)
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Which audience a snapshot is for. The difference is the whole privacy contract of
/// `PollState`: a chat-wide snapshot carries no viewer-specific data.
#[derive(Clone, Copy, Debug)]
pub enum Audience {
    /// `poll_updated` frames: `chosen` absent, quiz answer only once closed.
    Chat,
    /// One account's read: its own `chosen`, and the quiz answer once it has answered.
    Viewer(Uuid),
}

impl AppState {
    /// Snapshots of every poll among `message_ids`, keyed by message id. Ids that are not
    /// polls are simply absent. Callers authorise the chat before calling.
    pub(crate) async fn poll_states(
        &self,
        message_ids: &[Uuid],
        audience: Audience,
    ) -> Result<HashMap<Uuid, PollState>, sqlx::Error> {
        if message_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let list = placeholders(0, message_ids.len());
        let polls_sql = format!(
            "SELECT message_id, question, public_voters, multiple_choice, quiz, correct_option, \
             explanation, revision, closed_at FROM polls WHERE message_id IN ({list})"
        );
        let polls: Vec<PollRow> = with_pool!(self, |pool| {
            let mut query = sqlx::query_as(&polls_sql);
            for id in message_ids {
                query = query.bind(*id);
            }
            query.fetch_all(pool).await
        })?;
        if polls.is_empty() {
            return Ok(HashMap::new());
        }
        let ids: Vec<Uuid> = polls.iter().map(|row| row.0).collect();
        let list = placeholders(0, ids.len());
        let options_sql = format!(
            "SELECT poll_options.message_id, poll_options.position, poll_options.text, \
             COUNT(poll_votes.user_id) FROM poll_options \
             LEFT JOIN poll_votes ON poll_votes.message_id = poll_options.message_id \
               AND poll_votes.position = poll_options.position \
             WHERE poll_options.message_id IN ({list}) \
             GROUP BY poll_options.message_id, poll_options.position, poll_options.text \
             ORDER BY poll_options.message_id, poll_options.position"
        );
        let totals_sql = format!(
            "SELECT message_id, COUNT(DISTINCT user_id) FROM poll_votes \
             WHERE message_id IN ({list}) GROUP BY message_id"
        );
        let chosen_sql = format!(
            "SELECT message_id, position FROM poll_votes \
             WHERE user_id = $1 AND message_id IN ({}) ORDER BY position",
            placeholders(1, ids.len())
        );
        let (options, totals, chosen): AggregateRows = with_pool!(self, |pool| {
            let mut options = sqlx::query_as(&options_sql);
            let mut totals = sqlx::query_as(&totals_sql);
            for id in &ids {
                options = options.bind(*id);
                totals = totals.bind(*id);
            }
            let chosen = match audience {
                Audience::Chat => Vec::new(),
                Audience::Viewer(viewer) => {
                    let mut chosen = sqlx::query_as(&chosen_sql).bind(viewer);
                    for id in &ids {
                        chosen = chosen.bind(*id);
                    }
                    chosen.fetch_all(pool).await?
                }
            };
            Ok::<_, sqlx::Error>((
                options.fetch_all(pool).await?,
                totals.fetch_all(pool).await?,
                chosen,
            ))
        })?;

        let totals: HashMap<Uuid, i64> = totals.into_iter().collect();
        let mut options_by_poll: HashMap<Uuid, Vec<PollOption>> = HashMap::new();
        for (message_id, _, text, voters) in options {
            options_by_poll
                .entry(message_id)
                .or_default()
                .push(PollOption { text, voters });
        }
        let mut chosen_by_poll: HashMap<Uuid, Vec<u32>> = HashMap::new();
        for (message_id, position) in chosen {
            chosen_by_poll
                .entry(message_id)
                .or_default()
                .push(position as u32);
        }

        Ok(polls
            .into_iter()
            .map(|row| {
                let (id, question, public_voters, multiple_choice, quiz) =
                    (row.0, row.1, row.2, row.3, row.4);
                let closed = row.8.is_some();
                let chosen = match audience {
                    Audience::Chat => None,
                    Audience::Viewer(_) => Some(chosen_by_poll.remove(&id).unwrap_or_default()),
                };
                let answered = chosen.as_ref().is_some_and(|chosen| !chosen.is_empty());
                let reveal = quiz && (closed || answered);
                let state = PollState {
                    id,
                    question,
                    closed,
                    total_voters: totals.get(&id).copied().unwrap_or(0),
                    options: options_by_poll.remove(&id).unwrap_or_default(),
                    public_voters,
                    multiple_choice,
                    quiz,
                    correct_option: row.5.filter(|_| reveal).map(|index| index as u32),
                    explanation: row.6.filter(|_| reveal),
                    chosen,
                    revision: row.7,
                };
                (id, state)
            })
            .collect())
    }

    /// Fill `poll` on every non-recalled message that carries one, for `viewer`.
    ///
    /// Runs *after* the Redis history cache (see `message_history`), so cached pages never
    /// hold vote counts and a vote never has to invalidate the cache.
    pub(crate) async fn attach_message_polls(
        &self,
        messages: &mut [StoredMessage],
        viewer: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.recalled_at.is_none())
            .map(|message| message.id)
            .collect();
        let audience = viewer.map_or(Audience::Chat, Audience::Viewer);
        let mut states = self.poll_states(&ids, audience).await?;
        for message in messages.iter_mut() {
            message.poll = states.remove(&message.id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::placeholders;

    #[test]
    fn placeholders_number_after_the_offset() {
        assert_eq!(placeholders(0, 3), "$1, $2, $3");
        assert_eq!(placeholders(1, 2), "$2, $3");
    }
}
