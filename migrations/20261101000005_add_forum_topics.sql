-- TG-204: forum topics.
--
-- `messages.topic_id IS NULL` means the chat's General topic (Telegram's topic id 1): every
-- message written before this migration, and every message of a chat that is not a forum,
-- belongs to General without a backfill. A forum chat's General topic is still a row here
-- (`is_general`), holding its title, closed and hidden state; at most one per chat.
--
-- FK column spelling `room_id` per CONTEXT.md ("Room") and TG-004's frozen non-rename.
CREATE TABLE forum_topics (
    id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    is_general BOOLEAN NOT NULL DEFAULT FALSE,
    title TEXT NOT NULL,
    icon_emoji TEXT NOT NULL DEFAULT '',
    icon_custom_emoji_id TEXT,
    icon_color INTEGER NOT NULL,
    pinned_at TEXT,
    closed_at TEXT,
    is_hidden BOOLEAN NOT NULL DEFAULT FALSE,
    creator_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX forum_topics_room_idx ON forum_topics (room_id, created_at);
CREATE UNIQUE INDEX forum_topics_general_idx ON forum_topics (room_id) WHERE is_general;

-- A deleted topic takes its messages with it (the domain deletes them first so it can
-- recompute attachment orphan state; the cascade only catches a racing insert).
ALTER TABLE messages ADD COLUMN topic_id TEXT REFERENCES forum_topics (id) ON DELETE CASCADE;

-- Topic history pages and per-topic unread counts, General (`topic_id IS NULL`) included.
CREATE INDEX messages_room_topic_cursor_idx ON messages (room_id, topic_id, created_at, id);

-- One account's state in one topic: its read cursor and its mute, both independent of the
-- chat-level `chat_reads` cursor and `chat_members.muted_until`. The cursor is stored by value
-- (not an FK to messages) so deleting the read message does not reset it.
CREATE TABLE forum_topic_members (
    topic_id TEXT NOT NULL REFERENCES forum_topics (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    read_created_at TEXT,
    read_message_id TEXT,
    muted BOOLEAN NOT NULL DEFAULT FALSE,
    muted_until TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (topic_id, user_id)
);

CREATE INDEX forum_topic_members_user_idx ON forum_topic_members (user_id);
