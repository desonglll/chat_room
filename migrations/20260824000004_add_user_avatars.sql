CREATE TABLE user_avatar_files (
    user_id TEXT PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    storage_key TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    updated_at TEXT NOT NULL
);

