ALTER TABLE favorites ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

CREATE TABLE favorite_collaborators (
    favorite_id TEXT NOT NULL REFERENCES favorites (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY (favorite_id, user_id)
);

CREATE INDEX favorite_collaborators_user_idx
ON favorite_collaborators (user_id, added_at DESC);
