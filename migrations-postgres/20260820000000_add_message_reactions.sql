CREATE TABLE message_reactions (
    message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (message_id, user_id, emoji)
);

CREATE INDEX message_reactions_message_idx ON message_reactions (message_id, created_at);
