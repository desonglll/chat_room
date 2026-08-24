CREATE TABLE favorites (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    source_message_id TEXT REFERENCES messages (id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK (kind IN ('message', 'video', 'manual')),
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    source_sender TEXT NOT NULL DEFAULT '',
    source_room_name TEXT NOT NULL DEFAULT '',
    attachment_id TEXT REFERENCES attachments (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX favorites_user_created_idx ON favorites (user_id, created_at DESC, id);
CREATE UNIQUE INDEX favorites_user_message_idx
ON favorites (user_id, source_message_id) WHERE source_message_id IS NOT NULL;

