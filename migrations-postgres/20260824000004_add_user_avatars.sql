CREATE TABLE user_avatar_files (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    storage_key TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    updated_at TIMESTAMPTZ NOT NULL
);

