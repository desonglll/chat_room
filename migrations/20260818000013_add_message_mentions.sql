CREATE TABLE message_mentions (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    mentioned_user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (message_id, mentioned_user_id)
);

CREATE INDEX message_mentions_user_idx ON message_mentions (mentioned_user_id, created_at);
