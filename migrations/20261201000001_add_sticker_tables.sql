-- TG-302: sticker sets, stickers, and each account's sticker library.
--
-- Sticker files live in the attachment object store under their SHA-256 (`storage_key`),
-- the same content-addressed space as attachments, so identical bytes are stored once.
-- A sticker sent into a chat gets its own `attachments` row (per-message capability key);
-- `stickers.access_key` only authorizes the public set catalogue.
--
-- Stickers are soft-deleted (`removed_at`) so messages that reference them keep resolving
-- their set and emoji. `custom_emoji` is created here for TG-304, which owns its behaviour.
CREATE TABLE sticker_sets (
    id TEXT PRIMARY KEY NOT NULL,
    short_name TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    set_type TEXT NOT NULL DEFAULT 'regular',
    owner_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX sticker_sets_owner_idx ON sticker_sets (owner_id);

CREATE TABLE stickers (
    id TEXT PRIMARY KEY NOT NULL,
    set_id TEXT NOT NULL REFERENCES sticker_sets (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    emoji TEXT NOT NULL,
    format TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    duration_ms INTEGER,
    size_bytes INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    storage_key TEXT NOT NULL,
    access_key TEXT NOT NULL,
    created_at TEXT NOT NULL,
    removed_at TEXT
);

CREATE INDEX stickers_set_position_idx ON stickers (set_id, position);
-- The attachment orphan sweep asks "does any live sticker still use this object?".
CREATE INDEX stickers_storage_key_idx ON stickers (storage_key);

CREATE TABLE sticker_emojis (
    sticker_id TEXT NOT NULL REFERENCES stickers (id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (sticker_id, emoji)
);

CREATE INDEX sticker_emojis_emoji_idx ON sticker_emojis (emoji);

-- One row per account that has ever changed its library. Upserting it is the first write of
-- every library transaction, which serialises one account's concurrent installs/reorders.
CREATE TABLE user_sticker_state (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    revision INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE user_sticker_sets (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    set_id TEXT NOT NULL REFERENCES sticker_sets (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    archived_at TEXT,
    installed_at TEXT NOT NULL,
    PRIMARY KEY (user_id, set_id)
);

CREATE INDEX user_sticker_sets_order_idx ON user_sticker_sets (user_id, position);

CREATE TABLE user_recent_stickers (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    sticker_id TEXT NOT NULL REFERENCES stickers (id) ON DELETE CASCADE,
    used_at TEXT NOT NULL,
    PRIMARY KEY (user_id, sticker_id)
);

CREATE INDEX user_recent_stickers_used_idx ON user_recent_stickers (user_id, used_at DESC);

CREATE TABLE user_favorite_stickers (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    sticker_id TEXT NOT NULL REFERENCES stickers (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, sticker_id)
);

CREATE INDEX user_favorite_stickers_created_idx
    ON user_favorite_stickers (user_id, created_at DESC);

CREATE TABLE custom_emoji (
    sticker_id TEXT PRIMARY KEY NOT NULL REFERENCES stickers (id) ON DELETE CASCADE,
    set_id TEXT NOT NULL REFERENCES sticker_sets (id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX custom_emoji_set_idx ON custom_emoji (set_id);
