-- TG-305: saved GIFs and animation geometry.
--
-- A saved GIF is the account's own reference to a content-addressed object in the
-- attachment store (like a favorite): it is created only from a message the account could
-- read at that moment, and it survives the source message's recall. `storage_key` is
-- counted as a live reference by the attachment orphan sweep, so the object is kept while
-- any account still has it saved. `access_key` is the capability for
-- `GET /api/gifs/saved/:id/file`. `used_at` orders the list (sending or re-saving moves a
-- GIF to the front). No foreign key to `messages`: the source is informational only.
--
-- `animation_metadata` records the geometry the server read from an uploaded animation's
-- header, keyed by content hash, so the GIF panel can lay out a masonry grid before any
-- file is fetched. Rows are never deleted: they describe bytes, not a message.
CREATE TABLE user_saved_gifs (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    storage_key TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    access_key TEXT NOT NULL,
    source_message_id TEXT,
    saved_at TEXT NOT NULL,
    used_at TEXT NOT NULL,
    UNIQUE (user_id, content_hash)
);

CREATE INDEX user_saved_gifs_used_idx ON user_saved_gifs (user_id, used_at DESC);
CREATE INDEX user_saved_gifs_storage_idx ON user_saved_gifs (storage_key);

CREATE TABLE animation_metadata (
    content_hash TEXT PRIMARY KEY NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    duration_ms INTEGER,
    created_at TEXT NOT NULL
);

-- The "recent GIFs from my chats" query reads newest GIF messages first.
CREATE INDEX messages_gif_created_idx ON messages (created_at) WHERE media_kind = 'gif';
CREATE INDEX attachments_gif_mime_idx ON attachments (created_at) WHERE mime_type = 'image/gif';
