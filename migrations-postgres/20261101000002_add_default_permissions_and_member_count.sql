-- PostgreSQL twin of migrations/20261101000002_add_default_permissions_and_member_count.sql.
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

-- Statement-level triggers with transition tables rather than row-level ones: a bulk insert
-- of N members updates each chats row once, not N times. (A row trigger rewriting the same
-- chats row N times in one transaction walks an ever longer HOT chain.) INSERT ... ON
-- CONFLICT DO UPDATE fires both the INSERT and the UPDATE statement trigger, each with its
-- own transition table, so an upsert is counted exactly once.
CREATE FUNCTION chat_members_count_apply() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE chats SET member_count = chats.member_count + delta.n
        FROM (SELECT room_id, COUNT(*) AS n FROM new_members
              WHERE status = 'active' GROUP BY room_id) AS delta
        WHERE chats.id = delta.room_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE chats SET member_count = chats.member_count - delta.n
        FROM (SELECT room_id, COUNT(*) AS n FROM old_members
              WHERE status = 'active' GROUP BY room_id) AS delta
        WHERE chats.id = delta.room_id;
    ELSE
        UPDATE chats SET member_count = chats.member_count + delta.n
        FROM (SELECT room_id, SUM(n) AS n FROM (
                  SELECT room_id, 1 AS n FROM new_members WHERE status = 'active'
                  UNION ALL
                  SELECT room_id, -1 AS n FROM old_members WHERE status = 'active'
              ) AS changes GROUP BY room_id HAVING SUM(n) <> 0) AS delta
        WHERE chats.id = delta.room_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER chat_members_count_insert AFTER INSERT ON chat_members
REFERENCING NEW TABLE AS new_members
FOR EACH STATEMENT EXECUTE FUNCTION chat_members_count_apply();

CREATE TRIGGER chat_members_count_delete AFTER DELETE ON chat_members
REFERENCING OLD TABLE AS old_members
FOR EACH STATEMENT EXECUTE FUNCTION chat_members_count_apply();

CREATE TRIGGER chat_members_count_update AFTER UPDATE ON chat_members
REFERENCING OLD TABLE AS old_members NEW TABLE AS new_members
FOR EACH STATEMENT EXECUTE FUNCTION chat_members_count_apply();
