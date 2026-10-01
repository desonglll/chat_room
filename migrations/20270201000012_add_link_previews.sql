-- TG-408: link previews. `link_previews` caches one fetch per URL (failures too, so a broken
-- link is not re-fetched for every message); `message_link_previews` says which URL a message
-- previews and whether its sender hid the card. The card itself is a projection of the cache.
CREATE TABLE link_previews (
    url TEXT PRIMARY KEY NOT NULL,
    ok BOOLEAN NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    site_name TEXT NOT NULL DEFAULT '',
    image_url TEXT,
    fetched_at TEXT NOT NULL
);

CREATE TABLE message_link_previews (
    message_id TEXT PRIMARY KEY NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    hidden BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX message_link_previews_url_idx ON message_link_previews (url);
