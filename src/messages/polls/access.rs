//! Who may do what with a poll. Every read and every write goes through [`AppState::poll_access`].

use uuid::Uuid;

use super::model::PollError;
use crate::state::{with_pool, AppState};

/// The immutable facts about a poll plus the caller's standing, resolved once per request.
#[derive(Debug, Clone)]
pub(crate) struct PollAccess {
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub public_voters: bool,
    pub multiple_choice: bool,
    pub quiz: bool,
    pub option_count: usize,
}

/// `(room_id, sender_id, public_voters, multiple_choice, quiz, option_count)`.
type AccessRow = (Uuid, Option<Uuid>, bool, bool, bool, i64);

impl AppState {
    /// Resolve a poll for `user_id`, refusing with the *same* `NotFound` whether the poll does
    /// not exist, its message was recalled, its chat is deleted, or the caller is not an active
    /// member — so the existence of a poll never leaks across the chat boundary.
    ///
    /// Active membership, not `message.send`, is the bar: a channel subscriber cannot send,
    /// yet votes (Telegram semantics). Closing and the voter list add their own checks.
    pub(crate) async fn poll_access(
        &self,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<PollAccess, PollError> {
        let row: Option<AccessRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT polls.room_id, messages.sender_id, polls.public_voters, \
                   polls.multiple_choice, polls.quiz, \
                   (SELECT COUNT(*) FROM poll_options WHERE poll_options.message_id = polls.message_id) \
                 FROM polls \
                 JOIN messages ON messages.id = polls.message_id AND messages.recalled_at IS NULL \
                 JOIN chats ON chats.id = polls.room_id AND chats.deleted_at IS NULL \
                 WHERE polls.message_id = $1 AND EXISTS (SELECT 1 FROM chat_members \
                   WHERE chat_members.room_id = polls.room_id AND chat_members.user_id = $2 \
                     AND chat_members.status = 'active')",
            )
            .bind(message_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        let (room_id, sender_id, public_voters, multiple_choice, quiz, option_count) =
            row.ok_or(PollError::NotFound)?;
        Ok(PollAccess {
            message_id,
            room_id,
            sender_id,
            public_voters,
            multiple_choice,
            quiz,
            option_count: option_count as usize,
        })
    }

    /// Telegram: the poll's author or a chat administrator may stop a poll. "Administrator"
    /// is the `message.pin` holder — the one moderation key admins and owners carry and plain
    /// members do not (`src/chats/provisioning.rs`); the creator shortcut covers the owner.
    pub(crate) async fn may_close_poll(
        &self,
        access: &PollAccess,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        if access.sender_id == Some(user_id) {
            return Ok(true);
        }
        self.has_chat_permission(access.room_id, user_id, "message.pin")
            .await
    }
}

impl PollAccess {
    /// Validate a ballot against this poll: in range, no duplicates, and exactly one option
    /// unless multiple-choice. Returns the sorted, de-duplicated option indexes.
    pub(crate) fn ballot(&self, options: &[u32]) -> Result<Vec<u32>, PollError> {
        let mut ballot = options.to_vec();
        ballot.sort_unstable();
        ballot.dedup();
        let fits = !ballot.is_empty()
            && ballot.len() == options.len()
            && ballot
                .iter()
                .all(|index| (*index as usize) < self.option_count)
            && (self.multiple_choice || ballot.len() == 1);
        fits.then_some(ballot).ok_or(PollError::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(multiple_choice: bool) -> PollAccess {
        PollAccess {
            message_id: Uuid::nil(),
            room_id: Uuid::nil(),
            sender_id: None,
            public_voters: false,
            multiple_choice,
            quiz: false,
            option_count: 3,
        }
    }

    #[test]
    fn a_ballot_must_fit_the_poll() {
        assert_eq!(access(false).ballot(&[2]).unwrap(), vec![2]);
        assert!(access(false).ballot(&[0, 1]).is_err(), "single choice");
        assert!(access(false).ballot(&[]).is_err(), "empty");
        assert!(access(false).ballot(&[3]).is_err(), "out of range");
        assert_eq!(access(true).ballot(&[2, 0]).unwrap(), vec![0, 2]);
        assert!(access(true).ballot(&[1, 1]).is_err(), "duplicate");
    }
}
