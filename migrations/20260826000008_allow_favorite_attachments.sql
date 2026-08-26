PRAGMA defer_foreign_keys = ON;

CREATE TABLE attachments_new (
    id TEXT PRIMARY KEY NOT NULL,
    access_key TEXT NOT NULL UNIQUE,
    room_id TEXT REFERENCES rooms (id) ON DELETE CASCADE,
    uploader_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    created_at TEXT NOT NULL,
    is_sensitive INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    storage_key TEXT,
    orphaned_at TEXT
);

INSERT INTO attachments_new (
    id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes,
    created_at, is_sensitive, content_hash, storage_key, orphaned_at
)
SELECT id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes,
       created_at, is_sensitive, content_hash, storage_key, orphaned_at
FROM attachments;

DROP TABLE attachments;
ALTER TABLE attachments_new RENAME TO attachments;

CREATE INDEX attachments_room_id_idx ON attachments (room_id, created_at, id);
CREATE INDEX attachments_content_hash_idx ON attachments (content_hash);
CREATE INDEX attachments_storage_key_idx ON attachments (storage_key);
