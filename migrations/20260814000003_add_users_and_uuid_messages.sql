CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);

CREATE TABLE messages_v2 (
    id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL,
    sender_id TEXT,
    sender TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (sender_id) REFERENCES users (id) ON DELETE SET NULL
);

INSERT INTO messages_v2 (id, room_id, sender_id, sender, content, created_at)
SELECT printf('00000000-0000-0000-0000-%012x', id), room_id, NULL, sender, content, created_at
FROM messages;

DROP TABLE messages;
ALTER TABLE messages_v2 RENAME TO messages;

CREATE INDEX messages_room_cursor_idx ON messages (room_id, created_at, id);
