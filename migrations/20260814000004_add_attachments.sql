CREATE TABLE attachments (
    id TEXT PRIMARY KEY NOT NULL,
    access_key TEXT NOT NULL UNIQUE,
    room_id TEXT NOT NULL,
    uploader_id TEXT NOT NULL,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    data BLOB NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (uploader_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE INDEX attachments_room_id_idx ON attachments (room_id, created_at, id);

ALTER TABLE messages
ADD COLUMN attachment_id TEXT REFERENCES attachments (id) ON DELETE SET NULL;

CREATE INDEX messages_attachment_id_idx ON messages (attachment_id);
