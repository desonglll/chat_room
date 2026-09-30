-- TG-304: message entities (inline custom emoji now; bold/italic/code/links/mentions later)
-- and each account's emoji status.
--
-- Entities are a projection of the message text, stored beside it: `offset_utf16` and
-- `length_utf16` index `messages.content` in UTF-16 code units (Telegram's and JavaScript's
-- string semantics). One row per entity, ordered by `position`. Existing messages have no
-- rows, which means "plain text" — nothing is backfilled.
--
-- A custom emoji entity points at `custom_emoji(sticker_id)`. Stickers are soft-deleted, so
-- the reference normally survives; if the sticker row is ever hard-deleted the id becomes
-- NULL and clients show the fallback emoji that is part of the text itself.
CREATE TABLE message_entities (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    entity_type TEXT NOT NULL,
    offset_utf16 INTEGER NOT NULL,
    length_utf16 INTEGER NOT NULL,
    custom_emoji_id TEXT REFERENCES custom_emoji (sticker_id) ON DELETE SET NULL,
    url TEXT,
    user_id TEXT,
    language TEXT,
    PRIMARY KEY (message_id, position)
);

CREATE INDEX message_entities_custom_emoji_idx
    ON message_entities (custom_emoji_id) WHERE custom_emoji_id IS NOT NULL;

-- One status per account. Expired rows are filtered at read time, never trusted.
CREATE TABLE user_emoji_status (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    custom_emoji_id TEXT NOT NULL REFERENCES custom_emoji (sticker_id) ON DELETE CASCADE,
    expires_at TEXT,
    updated_at TEXT NOT NULL
);
