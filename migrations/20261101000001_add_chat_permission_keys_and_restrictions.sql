-- TG-201: the M2 permission keys, per-member restrictions with expiry, and per-admin rights.
-- docs/tg/architecture.md §4.4 — the registry is extended; there is no bitflag set.
--
-- The FK column spelling is `room_id`, not `chat_id`: CONTEXT.md ("Room") and TG-004's
-- frozen-non-rename keep `room_id` as the single name of a foreign key to chats(id).

INSERT INTO chat_permissions (permission_key, description) VALUES
    ('message.post', 'Publish posts in a channel'),
    ('message.edit_any', 'Edit messages sent by other accounts'),
    ('message.delete_any', 'Delete messages sent by other accounts'),
    ('message.pin', 'Pin and unpin chat messages'),
    ('message.send_media', 'Send photos, videos, files and voice messages'),
    ('message.send_sticker', 'Send stickers and GIFs'),
    ('message.send_poll', 'Start polls'),
    ('message.embed_link', 'Send link previews'),
    ('members.ban', 'Ban and restrict members'),
    ('members.promote', 'Appoint and dismiss administrators'),
    ('chat.info', 'Change the chat title, avatar and description'),
    ('chat.topics', 'Manage forum topics'),
    ('chat.anonymous', 'Post anonymously as the chat'),
    ('chat.call', 'Manage voice chats (reserved, not implemented)')
ON CONFLICT (permission_key) DO NOTHING;

-- The owner role holds every registered key.
INSERT INTO chat_role_permissions (role_id, permission_key)
SELECT chat_roles.id, chat_permissions.permission_key
FROM chat_roles CROSS JOIN chat_permissions
WHERE chat_roles.name = 'owner'
ON CONFLICT (role_id, permission_key) DO NOTHING;

-- A time-limited (or permanent, `until` NULL) denial of one key for one account. It is not a
-- role: it overrides a role grant for one person (decision step 4). It deliberately has no
-- foreign key to chat_members, so leaving and rejoining does not shed a restriction.
CREATE TABLE chat_member_restrictions (
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    denied_permission_key TEXT NOT NULL
        REFERENCES chat_permissions (permission_key) ON DELETE CASCADE,
    until TEXT,
    restricted_by TEXT REFERENCES users (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (room_id, user_id, denied_permission_key)
);

-- The background cleanup scans expired rows by time.
CREATE INDEX chat_member_restrictions_until_idx
ON chat_member_restrictions (until) WHERE until IS NOT NULL;

-- An administrator appointed with an explicit set of rights. When an admin has rows here they
-- replace the shared `admin` role's grants for that admin; with no rows the role applies.
-- The rows die with the membership.
CREATE TABLE chat_admin_rights (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    permission_key TEXT NOT NULL
        REFERENCES chat_permissions (permission_key) ON DELETE CASCADE,
    PRIMARY KEY (room_id, user_id, permission_key),
    FOREIGN KEY (room_id, user_id)
        REFERENCES chat_members (room_id, user_id) ON DELETE CASCADE
);

-- Telegram's admin "rank": a free-text badge shown next to an administrator.
ALTER TABLE chat_members ADD COLUMN custom_title TEXT NOT NULL DEFAULT '';

-- Keyset pagination of a 200 000-member roster: newest joiners first, ties by user id.
CREATE INDEX chat_members_room_joined_idx
ON chat_members (room_id, status, joined_at, user_id);

-- The administrator list is driven from the two admin-capable role ids, not by scanning the
-- whole roster.
CREATE INDEX chat_members_role_idx ON chat_members (role_id, status);
