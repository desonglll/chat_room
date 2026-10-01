-- TG-803: the link index behind the chat info panel's «链接» tab. A projection of message text
-- (messages stay the source of truth): one row per http(s) link per message. It is filled
-- lazily per chat when the tab is read — new messages after `chat_link_index_state`'s cursor,
-- and messages edited since `edits_seen_at` are re-extracted — so existing history is
-- backfilled on first read and no send path has to know about it.
CREATE TABLE message_links (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (message_id, position)
);

CREATE INDEX message_links_room_idx ON message_links (room_id, created_at, message_id, position);

CREATE TABLE chat_link_index_state (
    room_id TEXT PRIMARY KEY NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    indexed_created_at TEXT NOT NULL,
    indexed_message_id TEXT NOT NULL,
    edits_seen_at TEXT NOT NULL
);
