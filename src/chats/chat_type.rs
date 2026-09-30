//! The four chat types and the constraints intrinsic to each of them.
//!
//! Semantics come from `docs/tg/architecture.md` §4.3. Behaviour that depends on a type
//! (forum topics, channel posting, slow mode) arrives in M2; what lands here in M0 is the
//! vocabulary plus the intrinsic-constraint layer of the authorization decision, so that M1
//! can shape its UI around four types from the first screen instead of retrofitting them.

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// What kind of conversation a chat is. Stored as a lower-case string in `chats.chat_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    /// Exactly two participants. `direct_conversations` is the side table that finds it.
    Private,
    /// A small group. The historical shape of every `rooms` row before TG-004.
    #[default]
    Group,
    /// A large group: public handles, forum topics and slow mode live here.
    Supergroup,
    /// Broadcast. Only holders of `message.post` write; subscribers read.
    Channel,
}

/// Rejected when a string outside the `chats.chat_type` CHECK constraint is decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownChatType(pub String);

impl fmt::Display for UnknownChatType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown chat type: {}", self.0)
    }
}

impl std::error::Error for UnknownChatType {}

impl ChatType {
    pub const ALL: [ChatType; 4] = [
        ChatType::Private,
        ChatType::Group,
        ChatType::Supergroup,
        ChatType::Channel,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ChatType::Private => "private",
            ChatType::Group => "group",
            ChatType::Supergroup => "supergroup",
            ChatType::Channel => "channel",
        }
    }

    /// `None` means unbounded — a channel's subscriber count is not capped.
    pub const fn member_limit(self) -> Option<i64> {
        match self {
            ChatType::Private => Some(2),
            ChatType::Group => Some(200),
            ChatType::Supergroup => Some(200_000),
            ChatType::Channel => None,
        }
    }

    pub const fn allows_public_username(self) -> bool {
        matches!(self, ChatType::Supergroup | ChatType::Channel)
    }

    pub const fn allows_topics(self) -> bool {
        matches!(self, ChatType::Supergroup)
    }

    pub const fn allows_slow_mode(self) -> bool {
        matches!(self, ChatType::Supergroup)
    }

    pub const fn counts_views(self) -> bool {
        matches!(self, ChatType::Channel)
    }

    pub const fn allows_linked_discussion(self) -> bool {
        matches!(self, ChatType::Channel)
    }

    /// Only a supergroup lets an administrator choose what joiners see of the history.
    /// A private chat and a channel always show everything; a group never shows anything
    /// from before the join.
    pub const fn history_visibility_is_configurable(self) -> bool {
        matches!(self, ChatType::Supergroup)
    }

    /// The key a request for `permission_key` is decided on in this chat type. In a channel,
    /// sending *is* posting (TG-202): every send path asks for `message.send` (and the content
    /// kinds depend on it), and a channel answers that question with `message.post`. This is
    /// what lets a channel administrator send media, stickers and polls while a subscriber —
    /// whose role holds no key at all — can send nothing, on every path at once.
    pub fn effective_permission(self, permission_key: &str) -> &str {
        match (self, permission_key) {
            (ChatType::Channel, "message.send") => "message.post",
            _ => permission_key,
        }
    }

    /// The intrinsic-constraint layer of the authorization decision
    /// (`docs/tg/architecture.md` §4.4, step 5). It can only turn an allow into a deny:
    /// no chat type grants a permission that a role withheld.
    ///
    /// An unknown key is permitted. Whether a permission exists at all is the
    /// `chat_permissions` registry's decision, not this function's — returning `false` here
    /// for unknown keys would silently deny every permission M2 adds.
    pub fn permits(self, permission_key: &str) -> bool {
        match self {
            // A one-to-one chat has no roster to manage, no settings worth changing and
            // cannot be deleted out from under the other participant.
            ChatType::Private => !matches!(
                permission_key,
                "members.invite"
                    | "members.remove"
                    | "members.review"
                    | "members.roles"
                    | "members.ban"
                    | "members.promote"
                    | "room.settings"
                    | "room.delete"
                    | "chat.info"
                    | "chat.anonymous"
                    | "chat.call"
                    | "message.post"
                    | "chat.topics"
            ),
            ChatType::Group => !matches!(permission_key, "message.post" | "chat.topics"),
            ChatType::Supergroup => true,
            // Channels broadcast: `message.post` replaces `message.send`, and a channel is
            // not a forum.
            ChatType::Channel => !matches!(permission_key, "message.send" | "chat.topics"),
        }
    }
}

impl fmt::Display for ChatType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ChatType {
    type Err = UnknownChatType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "private" => Ok(ChatType::Private),
            "group" => Ok(ChatType::Group),
            "supergroup" => Ok(ChatType::Supergroup),
            "channel" => Ok(ChatType::Channel),
            other => Err(UnknownChatType(other.to_string())),
        }
    }
}

impl TryFrom<String> for ChatType {
    type Error = UnknownChatType;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<ChatType> for String {
    fn from(value: ChatType) -> Self {
        value.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_round_trips_through_its_stored_string() {
        for chat_type in ChatType::ALL {
            assert_eq!(chat_type.as_str().parse::<ChatType>(), Ok(chat_type));
        }
    }

    #[test]
    fn an_unknown_stored_string_is_rejected_rather_than_defaulted() {
        assert!("megagroup".parse::<ChatType>().is_err());
    }

    #[test]
    fn member_limits_match_the_architecture_table() {
        assert_eq!(ChatType::Private.member_limit(), Some(2));
        assert_eq!(ChatType::Group.member_limit(), Some(200));
        assert_eq!(ChatType::Supergroup.member_limit(), Some(200_000));
        assert_eq!(ChatType::Channel.member_limit(), None);
    }

    #[test]
    fn only_supergroups_and_channels_take_a_public_username() {
        assert!(!ChatType::Private.allows_public_username());
        assert!(!ChatType::Group.allows_public_username());
        assert!(ChatType::Supergroup.allows_public_username());
        assert!(ChatType::Channel.allows_public_username());
    }

    #[test]
    fn only_supergroups_take_topics_and_slow_mode() {
        for chat_type in ChatType::ALL {
            let supergroup = chat_type == ChatType::Supergroup;
            assert_eq!(chat_type.allows_topics(), supergroup);
            assert_eq!(chat_type.allows_slow_mode(), supergroup);
            assert_eq!(chat_type.history_visibility_is_configurable(), supergroup);
        }
    }

    #[test]
    fn only_channels_count_views_and_link_a_discussion_group() {
        for chat_type in ChatType::ALL {
            let channel = chat_type == ChatType::Channel;
            assert_eq!(chat_type.counts_views(), channel);
            assert_eq!(chat_type.allows_linked_discussion(), channel);
        }
    }

    #[test]
    fn a_private_chat_denies_every_roster_and_settings_permission() {
        for key in [
            "members.invite",
            "members.remove",
            "members.review",
            "members.roles",
            "members.ban",
            "members.promote",
            "room.settings",
            "room.delete",
            "chat.info",
        ] {
            assert!(!ChatType::Private.permits(key), "{key} should be denied");
            assert!(ChatType::Group.permits(key), "{key} should be allowed");
        }
    }

    #[test]
    fn a_channel_denies_plain_sending_and_a_group_denies_posting() {
        assert!(!ChatType::Channel.permits("message.send"));
        assert!(ChatType::Channel.permits("message.post"));
        assert!(ChatType::Group.permits("message.send"));
        assert!(!ChatType::Group.permits("message.post"));
    }

    #[test]
    fn a_channel_decides_sending_on_the_post_key() {
        assert_eq!(
            ChatType::Channel.effective_permission("message.send"),
            "message.post"
        );
        assert_eq!(
            ChatType::Channel.effective_permission("message.send_media"),
            "message.send_media"
        );
        for chat_type in [ChatType::Private, ChatType::Group, ChatType::Supergroup] {
            assert_eq!(
                chat_type.effective_permission("message.send"),
                "message.send"
            );
        }
    }

    #[test]
    fn a_supergroup_adds_no_intrinsic_denial() {
        for key in [
            "message.send",
            "message.post",
            "chat.topics",
            "members.roles",
            "room.delete",
        ] {
            assert!(ChatType::Supergroup.permits(key));
        }
    }

    #[test]
    fn an_unregistered_permission_key_is_left_to_the_registry() {
        for chat_type in ChatType::ALL {
            assert!(chat_type.permits("some.permission.m9.will.add"));
        }
    }
}
