-- TG-507: chat wallpapers, per account. `scope` is 'global' (every chat) or a chat id (that
-- chat only, overriding global). `kind` picks the source: a built-in preset, one colour, a
-- 2–4 colour gradient, or an uploaded image (stored like avatars, `image_key`). `blur` and
-- `dim` (0–80 %) apply to images. Deleting the row returns to the default.
CREATE TABLE chat_wallpapers (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('preset', 'color', 'gradient', 'image')),
    preset TEXT NOT NULL DEFAULT '',
    colors TEXT NOT NULL DEFAULT '[]',
    image_key TEXT,
    image_mime TEXT,
    image_size INTEGER,
    blur BOOLEAN NOT NULL DEFAULT FALSE,
    dim INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, scope)
);
