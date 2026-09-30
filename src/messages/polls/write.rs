//! Poll mutations. Each is one transaction whose first statement bumps `polls.revision`:
//! that write serialises every writer of the same poll on both adapters (SQLite takes its
//! write lock up front, PostgreSQL row-locks the poll), so a ballot's read-check-replace can
//! never interleave with another ballot for the same poll.

use chrono::Utc;
use uuid::Uuid;

use super::access::PollAccess;
use super::model::{PollError, PollVoter, PollVoterPage, ValidPoll};
use crate::message_store::MessagePlacement;
use crate::models::User;
use crate::state::{with_pool, AppState};

/// `(user_id, username, display_name, avatar_emoji, voted_at)`.
type VoterRow = (Uuid, String, String, String, chrono::DateTime<Utc>);

/// Copy a poll onto a forwarded message inside the forward's own transaction. Telegram
/// forwards a poll as a *new* poll: same question, options and mode, no votes, open, owned by
/// the forwarder. A no-op when the source message carries no poll.
/// Binds: `$1` new message id, `$2` target chat, `$3` forwarder, `$4` created_at, `$5` source.
pub(crate) const FORWARD_COPY_POLL: &str = "INSERT INTO polls \
    (message_id, room_id, creator_id, question, public_voters, multiple_choice, quiz, \
     correct_option, explanation, revision, closed_at, created_at) \
    SELECT $1, $2, $3, question, public_voters, multiple_choice, quiz, correct_option, \
      explanation, 0, NULL, $4 FROM polls WHERE message_id = $5";
/// Binds: `$1` new message id, `$2` source message id.
pub(crate) const FORWARD_COPY_POLL_OPTIONS: &str =
    "INSERT INTO poll_options (message_id, position, text) \
    SELECT $1, position, text FROM poll_options WHERE message_id = $2";

impl AppState {
    /// Insert the poll message and its poll atomically. Returns the message id, or `None` when
    /// the sender is not an active member at write time. A repeated `client_message_id`
    /// returns the first message's id and creates nothing.
    pub(crate) async fn insert_poll_message(
        &self,
        room_id: Uuid,
        sender: &User,
        display_name: &str,
        poll: &ValidPoll,
        placement: MessagePlacement,
        client_message_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self
            .reply_preview(room_id, placement.reply_to)
            .await?
            .map(|reply| reply.message_id);
        let inserted = with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let inserted = sqlx::query(
                    "INSERT INTO messages \
                     (id, room_id, sender_id, sender, content, reply_to_id, client_message_id, \
                      created_at, topic_id) \
                     SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9 \
                     WHERE EXISTS (SELECT 1 FROM chat_members WHERE chat_members.room_id = $2 \
                       AND chat_members.user_id = $3 AND chat_members.status = 'active') \
                     ON CONFLICT (room_id, sender_id, client_message_id) \
                     WHERE client_message_id IS NOT NULL DO NOTHING",
                )
                .bind(id)
                .bind(room_id)
                .bind(sender.id)
                .bind(display_name)
                .bind(&poll.question)
                .bind(reply_to)
                .bind(client_message_id)
                .bind(created_at)
                .bind(placement.topic_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected()
                    > 0;
                if !inserted {
                    return Ok::<_, sqlx::Error>(false);
                }
                sqlx::query(
                    "INSERT INTO polls (message_id, room_id, creator_id, question, \
                     public_voters, multiple_choice, quiz, correct_option, explanation, \
                     created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                )
                .bind(id)
                .bind(room_id)
                .bind(sender.id)
                .bind(&poll.question)
                .bind(poll.public_voters)
                .bind(poll.multiple_choice)
                .bind(poll.quiz)
                .bind(poll.correct_option.map(|index| index as i32))
                .bind(&poll.explanation)
                .bind(created_at)
                .execute(&mut *transaction)
                .await?;
                for (position, text) in poll.options.iter().enumerate() {
                    sqlx::query(
                        "INSERT INTO poll_options (message_id, position, text) VALUES ($1, $2, $3)",
                    )
                    .bind(id)
                    .bind(position as i32)
                    .bind(text)
                    .execute(&mut *transaction)
                    .await?;
                }
                transaction.commit().await?;
                Ok(true)
            }
            .await
        })?;
        if inserted {
            self.invalidate_message_cache(room_id).await;
            return Ok(Some(id));
        }
        let Some(client_message_id) = client_message_id else {
            return Ok(None);
        };
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM messages WHERE room_id = $1 AND sender_id = $2 \
                 AND client_message_id = $3",
            )
            .bind(room_id)
            .bind(sender.id)
            .bind(client_message_id)
            .fetch_optional(pool)
            .await
        })
    }

    /// Replace `user_id`'s ballot. A quiz ballot is final: a second one is refused.
    pub(crate) async fn record_poll_vote(
        &self,
        access: &PollAccess,
        user_id: Uuid,
        ballot: &[u32],
    ) -> Result<(), PollError> {
        let voted_at = Utc::now();
        with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let open = sqlx::query(
                    "UPDATE polls SET revision = revision + 1 \
                     WHERE message_id = $1 AND closed_at IS NULL",
                )
                .bind(access.message_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected()
                    > 0;
                if !open {
                    return Err(PollError::Closed);
                }
                if access.quiz {
                    let answered: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM poll_votes \
                         WHERE message_id = $1 AND user_id = $2)",
                    )
                    .bind(access.message_id)
                    .bind(user_id)
                    .fetch_one(&mut *transaction)
                    .await?;
                    if answered {
                        return Err(PollError::QuizAnswered);
                    }
                }
                sqlx::query("DELETE FROM poll_votes WHERE message_id = $1 AND user_id = $2")
                    .bind(access.message_id)
                    .bind(user_id)
                    .execute(&mut *transaction)
                    .await?;
                for position in ballot {
                    sqlx::query(
                        "INSERT INTO poll_votes (message_id, position, user_id, voted_at) \
                         VALUES ($1, $2, $3, $4)",
                    )
                    .bind(access.message_id)
                    .bind(*position as i32)
                    .bind(user_id)
                    .bind(voted_at)
                    .execute(&mut *transaction)
                    .await?;
                }
                transaction.commit().await?;
                Ok(())
            }
            .await
        })
    }

    /// Remove `user_id`'s ballot. Returns whether there was one; a quiz refuses.
    pub(crate) async fn retract_poll_vote(
        &self,
        access: &PollAccess,
        user_id: Uuid,
    ) -> Result<bool, PollError> {
        if access.quiz {
            return Err(PollError::QuizAnswered);
        }
        with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let open = sqlx::query(
                    "UPDATE polls SET revision = revision + 1 \
                     WHERE message_id = $1 AND closed_at IS NULL",
                )
                .bind(access.message_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected()
                    > 0;
                if !open {
                    return Err(PollError::Closed);
                }
                let removed =
                    sqlx::query("DELETE FROM poll_votes WHERE message_id = $1 AND user_id = $2")
                        .bind(access.message_id)
                        .bind(user_id)
                        .execute(&mut *transaction)
                        .await?
                        .rows_affected()
                        > 0;
                if removed {
                    transaction.commit().await?;
                }
                Ok(removed)
            }
            .await
        })
    }

    /// Close the poll. Returns whether this call closed it (closing twice is a no-op).
    pub(crate) async fn close_poll_row(&self, message_id: Uuid) -> Result<bool, sqlx::Error> {
        let closed_at = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE polls SET closed_at = $2, revision = revision + 1 \
                 WHERE message_id = $1 AND closed_at IS NULL",
            )
            .bind(message_id)
            .bind(closed_at)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    /// One page of one option's voters. Callers must have checked `public_voters`.
    pub(crate) async fn poll_voter_page(
        &self,
        message_id: Uuid,
        option: u32,
        limit: i64,
        offset: i64,
    ) -> Result<PollVoterPage, sqlx::Error> {
        let (total, rows): (i64, Vec<VoterRow>) = with_pool!(self, |pool| {
            let total = sqlx::query_scalar(
                "SELECT COUNT(*) FROM poll_votes WHERE message_id = $1 AND position = $2",
            )
            .bind(message_id)
            .bind(option as i32)
            .fetch_one(pool)
            .await?;
            let rows = sqlx::query_as(
                "SELECT users.id, users.username, users.display_name, users.avatar_emoji, \
                   poll_votes.voted_at FROM poll_votes \
                 JOIN users ON users.id = poll_votes.user_id \
                 WHERE poll_votes.message_id = $1 AND poll_votes.position = $2 \
                 ORDER BY poll_votes.voted_at DESC, users.id LIMIT $3 OFFSET $4",
            )
            .bind(message_id)
            .bind(option as i32)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            Ok::<_, sqlx::Error>((total, rows))
        })?;
        Ok(PollVoterPage {
            option,
            total,
            voters: rows
                .into_iter()
                .map(
                    |(user_id, username, display_name, avatar_emoji, voted_at)| PollVoter {
                        user_id,
                        username,
                        display_name,
                        avatar_emoji,
                        voted_at,
                    },
                )
                .collect(),
        })
    }
}
