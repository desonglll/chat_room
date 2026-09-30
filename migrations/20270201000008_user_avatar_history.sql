-- TG-511: profile photo history. `user_avatar_files` stays the "current photo" pointer every
-- existing reader uses; this table keeps every uploaded photo (Telegram lets you page through
-- them and set an older one as the main photo again). A photo's storage key is its id: it is
-- unique already, and knowing it grants nothing — downloads go through the privacy check.
CREATE TABLE user_avatar_history (
    storage_key TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0),
    created_at TEXT NOT NULL
);

CREATE INDEX user_avatar_history_user_idx ON user_avatar_history (user_id, created_at);

-- Every current photo becomes the first history entry.
INSERT INTO user_avatar_history (storage_key, user_id, mime_type, size_bytes, created_at)
SELECT storage_key, user_id, mime_type, size_bytes, updated_at FROM user_avatar_files;
