-- TG-1209: a link card's image, fetched by the server (through the TG-408 SSRF policy) and
-- served same-origin under an unguessable key, so the browser never loads third-party images
-- and the CSP keeps `img-src` to this origin. One image per cached page; it goes with the page.
CREATE TABLE link_preview_images (
    url TEXT PRIMARY KEY NOT NULL REFERENCES link_previews (url) ON DELETE CASCADE,
    access_key TEXT NOT NULL UNIQUE,
    content_type TEXT NOT NULL,
    data BLOB NOT NULL,
    fetched_at TEXT NOT NULL
);
