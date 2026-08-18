ALTER TABLE messages
ADD COLUMN reply_to_id TEXT REFERENCES messages (id) ON DELETE SET NULL;

CREATE INDEX messages_reply_to_id_idx ON messages (reply_to_id);

CREATE TABLE room_reads (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    read_at TEXT NOT NULL,
    PRIMARY KEY (room_id, user_id),
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (message_id) REFERENCES messages (id) ON DELETE CASCADE
);

CREATE INDEX room_reads_message_id_idx ON room_reads (message_id);
