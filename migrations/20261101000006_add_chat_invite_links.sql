-- TG-205: invite links. A chat holds one primary link plus any number of additional links,
-- each with an optional expiry, an optional usage limit, and an optional approval step.
--
-- `token` is the capability: random, unguessable, never derived from a row id. It is stored
-- in plain text because an administrator must be able to copy the link again (Telegram's
-- behaviour); every read of it is gated by the `members.invite` administrator check.
-- The FK column spelling is `room_id` (CONTEXT.md "Room", TG-004's frozen non-rename).
CREATE TABLE chat_invite_links (
    id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    token TEXT NOT NULL,
    creator_user_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    expires_at TEXT,
    usage_limit INTEGER CHECK (usage_limit IS NULL OR usage_limit > 0),
    usage_count INTEGER NOT NULL DEFAULT 0 CHECK (usage_count >= 0),
    requires_approval INTEGER NOT NULL DEFAULT 0,
    is_primary INTEGER NOT NULL DEFAULT 0,
    revoked_at TEXT,
    created_at TEXT NOT NULL,
    -- Telegram: a link either admits a bounded number of people or queues them for
    -- approval, never both.
    CHECK (NOT (requires_approval <> 0 AND usage_limit IS NOT NULL))
);

CREATE UNIQUE INDEX chat_invite_links_token_idx ON chat_invite_links (token);
CREATE INDEX chat_invite_links_room_idx ON chat_invite_links (room_id, created_at);
-- At most one live primary link per chat; a revoked primary stays for the record.
CREATE UNIQUE INDEX chat_invite_links_primary_idx ON chat_invite_links (room_id)
    WHERE is_primary <> 0 AND revoked_at IS NULL;

-- Which link admitted (or queued) a member: the per-link joined list and pending count.
-- A deleted link leaves its members in place.
ALTER TABLE chat_members ADD COLUMN invite_link_id TEXT
    REFERENCES chat_invite_links (id) ON DELETE SET NULL;
CREATE INDEX chat_members_invite_link_idx ON chat_members (invite_link_id, status);
