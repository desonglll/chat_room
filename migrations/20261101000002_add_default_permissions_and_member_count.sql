-- TG-201: group default permissions, the admin role's new default rights, and the
-- chats.member_count projection maintained by the database in the membership transaction.
--
-- Default permissions are the `member` role's grants (docs/tg/architecture.md §4.4 extends
-- the RBAC registry instead of adding a parallel mask). Every role that could send before
-- keeps being able to send media, stickers, polls and link previews, which are split out of
-- `message.send` from now on.
INSERT INTO chat_role_permissions (role_id, permission_key)
SELECT chat_role_permissions.role_id, new_keys.permission_key
FROM chat_role_permissions
CROSS JOIN (
    SELECT permission_key FROM chat_permissions WHERE permission_key IN
        ('message.send_media', 'message.send_sticker', 'message.send_poll', 'message.embed_link')
) AS new_keys
WHERE chat_role_permissions.permission_key = 'message.send'
ON CONFLICT (role_id, permission_key) DO NOTHING;

-- The shared `admin` role is the default right set of an administrator appointed without an
-- explicit selection. It gains the M2 moderation keys an admin had implicitly before.
INSERT INTO chat_role_permissions (role_id, permission_key)
SELECT chat_roles.id, chat_permissions.permission_key
FROM chat_roles CROSS JOIN chat_permissions
WHERE chat_roles.name = 'admin' AND chat_permissions.permission_key IN
    ('chat.info', 'message.delete_any', 'members.ban', 'chat.topics')
ON CONFLICT (role_id, permission_key) DO NOTHING;

-- Keyset pagination orders active members by joined_at; an active row always has one.
UPDATE chat_members SET joined_at = requested_at
WHERE status = 'active' AND joined_at IS NULL;

-- member_count counts active memberships. It was seeded at creation and never maintained
-- afterwards (joins, leaves, kicks and bans left it at 1). Recount once, then let triggers
-- keep it in the same transaction as every membership write, whichever module performs it
-- (including ON DELETE CASCADE from users).
UPDATE chats SET member_count = (
    SELECT COUNT(*) FROM chat_members
    WHERE chat_members.room_id = chats.id AND chat_members.status = 'active'
);

CREATE TRIGGER chat_members_count_insert AFTER INSERT ON chat_members
WHEN NEW.status = 'active'
BEGIN
    UPDATE chats SET member_count = member_count + 1 WHERE id = NEW.room_id;
END;

CREATE TRIGGER chat_members_count_delete AFTER DELETE ON chat_members
WHEN OLD.status = 'active'
BEGIN
    UPDATE chats SET member_count = member_count - 1 WHERE id = OLD.room_id;
END;

CREATE TRIGGER chat_members_count_update AFTER UPDATE OF status ON chat_members
WHEN (OLD.status = 'active') <> (NEW.status = 'active')
BEGIN
    UPDATE chats
    SET member_count = member_count + (CASE WHEN NEW.status = 'active' THEN 1 ELSE -1 END)
    WHERE id = NEW.room_id;
END;
