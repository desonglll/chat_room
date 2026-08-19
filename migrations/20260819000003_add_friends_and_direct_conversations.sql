CREATE TABLE friendships (
    user_low_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_high_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    requested_by_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    accepted_at TEXT,
    PRIMARY KEY (user_low_id, user_high_id),
    CHECK (user_low_id <> user_high_id),
    CHECK (requested_by_id = user_low_id OR requested_by_id = user_high_id)
);

CREATE INDEX friendships_high_status_idx ON friendships (user_high_id, status);
CREATE INDEX friendships_requester_status_idx ON friendships (requested_by_id, status);

CREATE TABLE user_blocks (
    blocker_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    blocked_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);

CREATE INDEX user_blocks_blocked_idx ON user_blocks (blocked_id);

CREATE TABLE direct_conversations (
    room_id TEXT PRIMARY KEY REFERENCES rooms (id) ON DELETE CASCADE,
    user_low_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_high_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    UNIQUE (user_low_id, user_high_id),
    CHECK (user_low_id <> user_high_id)
);

CREATE INDEX direct_conversations_high_idx ON direct_conversations (user_high_id);
