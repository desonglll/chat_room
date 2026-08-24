CREATE TABLE friend_remarks (
    owner_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    friend_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    remark TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (owner_id, friend_id),
    CHECK (owner_id <> friend_id)
);

