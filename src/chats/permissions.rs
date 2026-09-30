//! The chat permission registry as the code sees it (`docs/tg/architecture.md` §4.4).
//!
//! The database table `chat_permissions` is the registry; these constants say which part of
//! the registry each surface may touch. Nothing here is a bit mask: a permission is always a
//! registry key granted through a role, an administrator's explicit rights, or denied through
//! a per-member restriction.

use serde::Serialize;
use utoipa::ToSchema;

/// Who may ever hold a key: every member (subject to the group's default permissions), an
/// administrator (by appointment), or the owner alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PermissionScope {
    Member,
    Admin,
    Owner,
}

/// One registry key with its scope and the Chinese label the client shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct PermissionDescriptor {
    pub key: &'static str,
    pub scope: PermissionScope,
    pub label: &'static str,
}

const fn describe(
    key: &'static str,
    scope: PermissionScope,
    label: &'static str,
) -> PermissionDescriptor {
    PermissionDescriptor { key, scope, label }
}

/// Every registered key, in the order the client lists them. Nine keys predate M2; the other
/// fourteen are the M2 additions (`message.pin` was registered early by the pins feature).
pub const REGISTRY: &[PermissionDescriptor] = &[
    describe("message.send", PermissionScope::Member, "发送消息"),
    describe("message.send_media", PermissionScope::Member, "发送媒体"),
    describe(
        "message.send_sticker",
        PermissionScope::Member,
        "发送贴纸与 GIF",
    ),
    describe("message.send_poll", PermissionScope::Member, "发起投票"),
    describe("message.embed_link", PermissionScope::Member, "链接预览"),
    describe("members.invite", PermissionScope::Member, "添加成员"),
    describe("message.pin", PermissionScope::Member, "置顶消息"),
    describe("chat.info", PermissionScope::Member, "修改群信息"),
    describe("chat.topics", PermissionScope::Member, "管理话题"),
    describe(
        "message.edit_own",
        PermissionScope::Member,
        "编辑自己的消息",
    ),
    describe(
        "message.recall_own",
        PermissionScope::Member,
        "撤回自己的消息",
    ),
    describe("room.settings", PermissionScope::Admin, "修改群设置"),
    describe("message.post", PermissionScope::Admin, "发布频道消息"),
    describe("message.edit_any", PermissionScope::Admin, "编辑他人消息"),
    describe("message.delete_any", PermissionScope::Admin, "删除他人消息"),
    describe("members.review", PermissionScope::Admin, "审批入群申请"),
    describe("members.remove", PermissionScope::Admin, "移除成员"),
    describe("members.ban", PermissionScope::Admin, "封禁与限制成员"),
    describe("members.promote", PermissionScope::Admin, "任命管理员"),
    describe("chat.anonymous", PermissionScope::Admin, "匿名发言"),
    describe("chat.call", PermissionScope::Admin, "管理语音聊天"),
    describe("members.roles", PermissionScope::Owner, "修改成员角色"),
    describe("room.delete", PermissionScope::Owner, "删除群组"),
];

/// Keys a group's default permissions can switch on or off for every ordinary member, and
/// the same keys a per-member restriction can deny. Telegram's "成员权限" page.
pub const MEMBER_TOGGLEABLE: &[&str] = &[
    "message.send",
    "message.send_media",
    "message.send_sticker",
    "message.send_poll",
    "message.embed_link",
    "members.invite",
    "message.pin",
    "chat.info",
    "chat.topics",
];

/// The toggles a brand-new group starts with switched on.
pub const DEFAULT_MEMBER_PERMISSIONS: &[&str] = &[
    "message.send",
    "message.send_media",
    "message.send_sticker",
    "message.send_poll",
    "message.embed_link",
];

/// What every member always holds, whatever the defaults: acting on one's own messages.
pub const MEMBER_BASELINE: &[&str] = &["message.edit_own", "message.recall_own"];

/// What every administrator holds regardless of the rights they were appointed with: an
/// administrator is never subject to the group's default permissions.
pub const ADMIN_BASELINE: &[&str] = &[
    "message.send",
    "message.send_media",
    "message.send_sticker",
    "message.send_poll",
    "message.embed_link",
    "message.edit_own",
    "message.recall_own",
];

/// The checkboxes of the admin appointment form.
pub const ADMIN_ASSIGNABLE: &[&str] = &[
    "chat.info",
    "room.settings",
    "message.post",
    "message.edit_any",
    "message.delete_any",
    "message.pin",
    "members.review",
    "members.invite",
    "members.remove",
    "members.ban",
    "members.promote",
    "chat.topics",
    "chat.anonymous",
    "chat.call",
];

/// The rights of an administrator appointed without an explicit selection (the legacy
/// `set_role: admin` action, and every admin that existed before M2). They are the shared
/// `admin` role's grants beyond [`ADMIN_BASELINE`].
pub const DEFAULT_ADMIN_RIGHTS: &[&str] = &[
    "chat.info",
    "room.settings",
    "message.delete_any",
    "message.pin",
    "members.review",
    "members.invite",
    "members.remove",
    "members.ban",
    "chat.topics",
];

/// The member role's grants for a group created with default settings.
pub fn member_role_grants(defaults: &[&str]) -> Vec<&'static str> {
    MEMBER_BASELINE
        .iter()
        .copied()
        .chain(
            MEMBER_TOGGLEABLE
                .iter()
                .copied()
                .filter(|key| defaults.contains(key)),
        )
        .collect()
}

/// The shared admin role's grants.
pub fn admin_role_grants() -> Vec<&'static str> {
    ADMIN_BASELINE
        .iter()
        .chain(DEFAULT_ADMIN_RIGHTS)
        .copied()
        .collect()
}

/// The owner role's grants: the whole registry.
pub fn owner_role_grants() -> Vec<&'static str> {
    REGISTRY.iter().map(|descriptor| descriptor.key).collect()
}

/// Sending a particular kind of content also needs the right to send at all: a member who may
/// not send messages may not send a sticker either, whatever the sticker toggle says.
pub fn prerequisite(permission_key: &str) -> Option<&'static str> {
    match permission_key {
        "message.send_media"
        | "message.send_sticker"
        | "message.send_poll"
        | "message.embed_link" => Some("message.send"),
        _ => None,
    }
}

/// Resolve client-supplied keys against an allow-list, rejecting anything outside it.
/// Returns the keys as `'static` registry strings, deduplicated, in allow-list order.
pub fn validate_keys(
    requested: &[String],
    allowed: &[&'static str],
) -> Result<Vec<&'static str>, String> {
    if let Some(unknown) = requested
        .iter()
        .find(|key| !allowed.contains(&key.as_str()))
    {
        return Err(unknown.clone());
    }
    Ok(allowed
        .iter()
        .copied()
        .filter(|key| requested.iter().any(|requested| requested == key))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_holds_the_nine_original_and_fourteen_m2_keys() {
        assert_eq!(REGISTRY.len(), 23);
        for key in [
            "message.post",
            "message.edit_any",
            "message.delete_any",
            "message.pin",
            "message.send_media",
            "message.send_sticker",
            "message.send_poll",
            "message.embed_link",
            "members.ban",
            "members.promote",
            "chat.info",
            "chat.topics",
            "chat.anonymous",
            "chat.call",
        ] {
            assert!(REGISTRY.iter().any(|entry| entry.key == key), "{key}");
        }
    }

    #[test]
    fn every_listed_key_is_registered() {
        for list in [
            MEMBER_TOGGLEABLE,
            DEFAULT_MEMBER_PERMISSIONS,
            MEMBER_BASELINE,
            ADMIN_BASELINE,
            ADMIN_ASSIGNABLE,
            DEFAULT_ADMIN_RIGHTS,
        ] {
            for key in list {
                assert!(REGISTRY.iter().any(|entry| entry.key == *key), "{key}");
            }
        }
    }

    #[test]
    fn owner_only_keys_are_never_assignable_or_toggleable() {
        for key in ["members.roles", "room.delete"] {
            assert!(!ADMIN_ASSIGNABLE.contains(&key));
            assert!(!MEMBER_TOGGLEABLE.contains(&key));
        }
    }

    #[test]
    fn content_kinds_depend_on_sending() {
        assert_eq!(prerequisite("message.send_sticker"), Some("message.send"));
        assert_eq!(prerequisite("message.send"), None);
        assert_eq!(prerequisite("members.ban"), None);
    }

    #[test]
    fn validation_rejects_keys_outside_the_allow_list_and_deduplicates() {
        let requested = vec![
            "message.pin".to_string(),
            "message.send".to_string(),
            "message.pin".to_string(),
        ];
        assert_eq!(
            validate_keys(&requested, MEMBER_TOGGLEABLE),
            Ok(vec!["message.send", "message.pin"])
        );
        assert_eq!(
            validate_keys(&["room.delete".to_string()], ADMIN_ASSIGNABLE),
            Err("room.delete".to_string())
        );
    }

    #[test]
    fn member_grants_always_keep_the_baseline() {
        let grants = member_role_grants(&[]);
        assert_eq!(grants, vec!["message.edit_own", "message.recall_own"]);
    }
}
