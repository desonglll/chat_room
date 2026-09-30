//! The one-way `group` → `supergroup` upgrade.
//!
//! `docs/tg/architecture.md` §4.3: a group becomes a supergroup when it outgrows 200 members,
//! takes a public username, enables forum topics, or turns on slow mode. Telegram makes the
//! change irreversible and so does this module — there is no downgrade and none may be added,
//! because a supergroup's history and permission model cannot be folded back into a group's.

use super::chat_type::ChatType;

/// Which capability forced the upgrade. Recorded so the caller can tell the user why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupergroupUpgradeTrigger {
    /// Active members passed `ChatType::Group.member_limit()`.
    MemberLimitExceeded,
    /// A public `@username` was set, which only a supergroup or channel may hold.
    PublicUsernameSet,
    /// Forum topics were enabled.
    ForumEnabled,
    /// A non-zero slow mode was configured.
    SlowModeEnabled,
}

impl SupergroupUpgradeTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            SupergroupUpgradeTrigger::MemberLimitExceeded => "member_limit_exceeded",
            SupergroupUpgradeTrigger::PublicUsernameSet => "public_username_set",
            SupergroupUpgradeTrigger::ForumEnabled => "forum_enabled",
            SupergroupUpgradeTrigger::SlowModeEnabled => "slow_mode_enabled",
        }
    }
}

/// The chat state an upgrade decision is made against. Mirrors the `chats` columns that the
/// trigger conditions read, so a caller can evaluate a proposed change before writing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatCapabilityRequest {
    pub member_count: i64,
    pub has_public_username: bool,
    pub is_forum: bool,
    pub slow_mode_seconds: i64,
}

/// The first trigger the requested state satisfies, or `None` when the state needs no upgrade.
///
/// Only a `group` can be triggered. A `private` chat cannot grow into a supergroup (it is
/// capped at two members by definition and a direct conversation is not a group), and a
/// `supergroup` or `channel` already has every capability the triggers are about.
pub fn supergroup_upgrade_trigger(
    chat_type: ChatType,
    requested: &ChatCapabilityRequest,
) -> Option<SupergroupUpgradeTrigger> {
    if chat_type != ChatType::Group {
        return None;
    }
    let over_limit = chat_type
        .member_limit()
        .is_some_and(|limit| requested.member_count > limit);
    if over_limit {
        return Some(SupergroupUpgradeTrigger::MemberLimitExceeded);
    }
    if requested.has_public_username {
        return Some(SupergroupUpgradeTrigger::PublicUsernameSet);
    }
    if requested.is_forum {
        return Some(SupergroupUpgradeTrigger::ForumEnabled);
    }
    if requested.slow_mode_seconds > 0 {
        return Some(SupergroupUpgradeTrigger::SlowModeEnabled);
    }
    None
}

impl ChatType {
    /// The type this chat becomes when `trigger` fires, or `None` when nothing changes.
    ///
    /// `Some(Supergroup)` is returned for `Group` and for nothing else. The signature deliberately
    /// cannot express a downgrade.
    pub const fn upgraded(self, _trigger: SupergroupUpgradeTrigger) -> Option<ChatType> {
        match self {
            ChatType::Group => Some(ChatType::Supergroup),
            ChatType::Private | ChatType::Supergroup | ChatType::Channel => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_at(member_count: i64) -> ChatCapabilityRequest {
        ChatCapabilityRequest {
            member_count,
            ..ChatCapabilityRequest::default()
        }
    }

    #[test]
    fn a_group_at_the_limit_is_not_upgraded_but_one_over_it_is() {
        assert_eq!(
            supergroup_upgrade_trigger(ChatType::Group, &group_at(200)),
            None
        );
        assert_eq!(
            supergroup_upgrade_trigger(ChatType::Group, &group_at(201)),
            Some(SupergroupUpgradeTrigger::MemberLimitExceeded)
        );
    }

    #[test]
    fn each_of_the_four_documented_conditions_triggers_an_upgrade() {
        let cases = [
            (
                ChatCapabilityRequest {
                    member_count: 5_000,
                    ..Default::default()
                },
                SupergroupUpgradeTrigger::MemberLimitExceeded,
            ),
            (
                ChatCapabilityRequest {
                    has_public_username: true,
                    ..Default::default()
                },
                SupergroupUpgradeTrigger::PublicUsernameSet,
            ),
            (
                ChatCapabilityRequest {
                    is_forum: true,
                    ..Default::default()
                },
                SupergroupUpgradeTrigger::ForumEnabled,
            ),
            (
                ChatCapabilityRequest {
                    slow_mode_seconds: 30,
                    ..Default::default()
                },
                SupergroupUpgradeTrigger::SlowModeEnabled,
            ),
        ];
        for (requested, expected) in cases {
            assert_eq!(
                supergroup_upgrade_trigger(ChatType::Group, &requested),
                Some(expected)
            );
        }
    }

    #[test]
    fn an_untouched_group_is_left_alone() {
        assert_eq!(
            supergroup_upgrade_trigger(ChatType::Group, &ChatCapabilityRequest::default()),
            None
        );
    }

    #[test]
    fn no_other_type_is_ever_triggered() {
        let maximal = ChatCapabilityRequest {
            member_count: 1_000_000,
            has_public_username: true,
            is_forum: true,
            slow_mode_seconds: 60,
        };
        for chat_type in [ChatType::Private, ChatType::Supergroup, ChatType::Channel] {
            assert_eq!(supergroup_upgrade_trigger(chat_type, &maximal), None);
        }
    }

    #[test]
    fn the_upgrade_is_one_way() {
        let trigger = SupergroupUpgradeTrigger::ForumEnabled;
        assert_eq!(
            ChatType::Group.upgraded(trigger),
            Some(ChatType::Supergroup)
        );
        // Nothing downgrades, and a supergroup does not upgrade again.
        assert_eq!(ChatType::Supergroup.upgraded(trigger), None);
        assert_eq!(ChatType::Channel.upgraded(trigger), None);
        assert_eq!(ChatType::Private.upgraded(trigger), None);
    }

    #[test]
    fn an_upgraded_group_gains_exactly_the_capabilities_that_triggered_it() {
        let upgraded = ChatType::Group
            .upgraded(SupergroupUpgradeTrigger::PublicUsernameSet)
            .expect("a group upgrades");
        assert!(upgraded.allows_public_username());
        assert!(upgraded.allows_topics());
        assert!(upgraded.allows_slow_mode());
        assert!(!ChatType::Group.allows_public_username());
    }
}
