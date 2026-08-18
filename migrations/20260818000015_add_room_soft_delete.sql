-- Soft-delete rooms instead of hard-deleting them: messages/attachments stay
-- intact, the room just disappears from listings. SQLite can't drop the inline
-- UNIQUE(name) constraint directly, so rebuild the table with a partial unique
-- index that only applies to non-deleted rooms (letting names be reused).
PRAGMA defer_foreign_keys = ON;

CREATE TABLE rooms_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL DEFAULT '',
    creator_user_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    join_policy TEXT NOT NULL DEFAULT 'open' CHECK (join_policy IN ('open', 'approval')),
    avatar_emoji TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    deleted_at TEXT,
    created_at TEXT NOT NULL
);

INSERT INTO rooms_new (id, name, password_hash, creator_user_id, join_policy, avatar_emoji, description, deleted_at, created_at)
SELECT id, name, password_hash, creator_user_id, join_policy, avatar_emoji, description, NULL, created_at FROM rooms;

DROP TABLE rooms;
ALTER TABLE rooms_new RENAME TO rooms;

CREATE INDEX rooms_created_at_idx ON rooms (created_at, id);
CREATE UNIQUE INDEX rooms_name_active_idx ON rooms (name) WHERE deleted_at IS NULL;
