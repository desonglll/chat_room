ALTER TABLE users
ADD COLUMN avatar_emoji TEXT NOT NULL DEFAULT '';

ALTER TABLE messages
ADD COLUMN recalled_at TEXT;

CREATE INDEX messages_recalled_at_idx ON messages (room_id, recalled_at);

CREATE TABLE room_participants (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    joined_at TEXT NOT NULL,
    PRIMARY KEY (room_id, user_id),
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE INDEX room_participants_user_id_idx ON room_participants (user_id);
