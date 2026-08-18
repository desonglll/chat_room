CREATE TABLE message_mentions (
    message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    mentioned_user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (message_id, mentioned_user_id)
);

CREATE INDEX message_mentions_user_idx ON message_mentions (mentioned_user_id, created_at);
