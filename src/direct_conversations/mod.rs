//! The private-chat lookup index: an unordered pair of users → their `private` chat.
//!
//! TG-208 reduced this module to exactly that. A private chat is an ordinary chat
//! (`chats.chat_type = 'private'`) and every message read, message write, membership and
//! authorization decision for it goes through `crate::chats` like any other chat's.
//! `direct_conversations` only answers "which chat do these two users share" and "who is the
//! other participant"; it never decides whether something is private (that is `chat_type`)
//! and it never touches `messages`.

mod index;

pub use index::PrivateChatPeer;
pub(crate) use index::{chat_for_pair, record_pair};
