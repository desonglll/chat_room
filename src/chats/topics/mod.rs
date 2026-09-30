//! Forum topics (TG-204, `docs/devlog/TG-204.md`): a supergroup with `is_forum` splits its
//! messages into topics. General is a real row per forum, but its messages keep
//! `messages.topic_id = NULL`, so history from before the forum belongs to it untouched.
//!
//! - `model` — wire types, palette and validation;
//! - `store` — `forum_topics` rows: General, create, edit, delete (with its messages);
//! - `posting` — the rule every send path applies (`resolve_post_topic`);
//! - `reads` — per-topic read cursor, unread counts and mute;
//! - `history` — topic-scoped message pages;
//! - `view` — what one viewer sees, and `topic_updated` announcements;
//! - `handlers` — the HTTP surface (forum toggle and topic management);
//! - `viewer_handlers` — topic history, read cursor and mute.

pub mod handlers;
pub mod history;
pub mod model;
pub mod posting;
pub mod reads;
pub mod store;
pub mod view;
pub mod viewer_handlers;

pub use model::{ForumTopic, ForumTopicList, TopicError, TOPIC_COLORS};
