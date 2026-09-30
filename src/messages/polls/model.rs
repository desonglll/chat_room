//! Request/response shapes and validation for polls (TG-406).
//!
//! The poll snapshot itself (`PollState`) is a realtime payload owned by
//! `crate::realtime::payloads` because the frozen `poll_updated` frame carries it; everything
//! here is REST-only and lives beside the domain (`AGENTS.md`).

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Telegram's limits: 1–300 character question, 2–10 options of 1–100 characters, and a
/// quiz explanation of at most 200 characters.
pub const MAX_QUESTION_CHARS: usize = 300;
pub const MIN_OPTIONS: usize = 2;
pub const MAX_OPTIONS: usize = 10;
pub const MAX_OPTION_CHARS: usize = 100;
pub const MAX_EXPLANATION_CHARS: usize = 200;

/// `POST /api/chats/:id/polls`.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct CreatePollRequest {
    pub question: String,
    pub options: Vec<String>,
    /// `false` (the default, as in Telegram) is an anonymous poll.
    #[serde(default)]
    pub public_voters: bool,
    #[serde(default)]
    pub multiple_choice: bool,
    #[serde(default)]
    pub quiz: bool,
    /// Required for a quiz, refused otherwise.
    #[serde(default)]
    pub correct_option: Option<u32>,
    /// Quiz only.
    #[serde(default)]
    pub explanation: Option<String>,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    /// Idempotency key, same contract as the WebSocket `message` frame.
    #[serde(default)]
    pub client_message_id: Option<Uuid>,
    /// TG-204: the forum topic to post into; absent = General.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
}

/// A request that passed [`CreatePollRequest::validate`]: trimmed, bounded, consistent.
#[derive(Debug, Clone)]
pub struct ValidPoll {
    pub question: String,
    pub options: Vec<String>,
    pub public_voters: bool,
    pub multiple_choice: bool,
    pub quiz: bool,
    pub correct_option: Option<u32>,
    pub explanation: Option<String>,
}

impl CreatePollRequest {
    pub fn validate(&self) -> Result<ValidPoll, PollError> {
        let bounded = |text: &str, max: usize| {
            let trimmed = text.trim();
            let count = trimmed.chars().count();
            (count >= 1 && count <= max).then(|| trimmed.to_string())
        };
        let question = bounded(&self.question, MAX_QUESTION_CHARS).ok_or(PollError::Invalid)?;
        if !(MIN_OPTIONS..=MAX_OPTIONS).contains(&self.options.len()) {
            return Err(PollError::Invalid);
        }
        let options = self
            .options
            .iter()
            .map(|option| bounded(option, MAX_OPTION_CHARS))
            .collect::<Option<Vec<_>>>()
            .ok_or(PollError::Invalid)?;
        let explanation = match &self.explanation {
            Some(text) if !text.trim().is_empty() => {
                Some(bounded(text, MAX_EXPLANATION_CHARS).ok_or(PollError::Invalid)?)
            }
            _ => None,
        };
        if self.quiz {
            let correct = self.correct_option.ok_or(PollError::Invalid)?;
            if self.multiple_choice || correct as usize >= options.len() {
                return Err(PollError::Invalid);
            }
        } else if self.correct_option.is_some() || explanation.is_some() {
            return Err(PollError::Invalid);
        }
        Ok(ValidPoll {
            question,
            options,
            public_voters: self.public_voters,
            multiple_choice: self.multiple_choice,
            quiz: self.quiz,
            correct_option: self.correct_option.filter(|_| self.quiz),
            explanation,
        })
    }
}

/// `POST /api/polls/:message_id/votes`. Retracting is `DELETE` on the same path.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct VoteRequest {
    /// Option indexes. Exactly one unless the poll is multiple-choice.
    pub options: Vec<u32>,
}

/// `GET /api/polls/:message_id/voters?option=&limit=&offset=`.
#[derive(Debug, Clone, Deserialize)]
pub struct VotersQuery {
    pub option: u32,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PollVoter {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
    pub voted_at: DateTime<Utc>,
}

/// One page of one option's voters, newest first. Public polls only.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PollVoterPage {
    pub option: u32,
    pub total: i64,
    pub voters: Vec<PollVoter>,
}

/// Every way a poll action can fail. The HTTP mapping is in [`PollError::status`].
#[derive(Debug)]
pub enum PollError {
    /// No such poll, the message was recalled, or the caller cannot see the chat. One variant
    /// on purpose: a non-member learns nothing about whether the poll exists.
    NotFound,
    /// The caller can see the poll but may not do this (close someone else's poll, list the
    /// voters of an anonymous poll, create a poll without the send permission).
    Forbidden,
    /// Malformed poll, or option indexes that do not fit this poll.
    Invalid,
    /// The poll is closed; nothing about its votes can change.
    Closed,
    /// A quiz answer is final: it can be neither changed nor retracted.
    QuizAnswered,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for PollError {
    fn from(error: sqlx::Error) -> Self {
        PollError::Database(error)
    }
}

impl PollError {
    pub fn status(&self) -> StatusCode {
        match self {
            PollError::NotFound => StatusCode::NOT_FOUND,
            PollError::Forbidden => StatusCode::FORBIDDEN,
            PollError::Invalid => StatusCode::BAD_REQUEST,
            PollError::Closed | PollError::QuizAnswered => StatusCode::CONFLICT,
            PollError::Database(error) => {
                tracing::error!("poll operation failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(options: &[&str]) -> CreatePollRequest {
        CreatePollRequest {
            question: " Lunch? ".into(),
            options: options.iter().map(|o| o.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn trims_and_bounds_the_question_and_options() {
        let valid = request(&["yes", " no "]).validate().unwrap();
        assert_eq!(valid.question, "Lunch?");
        assert_eq!(valid.options, vec!["yes", "no"]);
        assert!(request(&["only one"]).validate().is_err());
        assert!(request(&["a", "  "]).validate().is_err());
        assert!(request(&["a"; 11]).validate().is_err());
        let long = "x".repeat(MAX_OPTION_CHARS + 1);
        assert!(request(&["a", &long]).validate().is_err());
    }

    #[test]
    fn a_quiz_needs_exactly_one_valid_correct_option() {
        let mut quiz = request(&["3", "4"]);
        quiz.quiz = true;
        assert!(quiz.validate().is_err(), "missing correct option");
        quiz.correct_option = Some(2);
        assert!(quiz.validate().is_err(), "out of range");
        quiz.correct_option = Some(1);
        quiz.explanation = Some("2+2".into());
        assert_eq!(quiz.validate().unwrap().correct_option, Some(1));
        quiz.multiple_choice = true;
        assert!(quiz.validate().is_err(), "a quiz is single choice");
    }

    #[test]
    fn a_regular_poll_refuses_quiz_fields() {
        let mut poll = request(&["a", "b"]);
        poll.correct_option = Some(0);
        assert!(poll.validate().is_err());
        poll.correct_option = None;
        poll.explanation = Some("why".into());
        assert!(poll.validate().is_err());
    }
}
