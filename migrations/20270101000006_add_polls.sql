-- TG-406: polls and quizzes.
--
-- A poll is keyed by the message that carries it (`polls.message_id`), not by a separate id:
-- the message row stays the source of truth for chat, sender, time, recall and history order,
-- and this table only adds the poll projection beside it. TG-302 owns `messages.media_kind`
-- (20261201000002); this migration deliberately does not depend on that column — a message
-- is a poll exactly when a `polls` row exists for it.
--
-- Vote counts are never denormalised: they are `COUNT(*)` over `poll_votes`, so a counter can
-- never drift from the votes it summarises. `revision` is bumped by every vote/retract/close
-- transaction; its UPDATE is the first statement of that transaction, which serialises all
-- writers of one poll (SQLite: takes the write lock up front; PostgreSQL: row lock).
CREATE TABLE polls (
    message_id TEXT PRIMARY KEY REFERENCES messages (id) ON DELETE CASCADE,
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    creator_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    question TEXT NOT NULL,
    public_voters BOOLEAN NOT NULL DEFAULT FALSE,
    multiple_choice BOOLEAN NOT NULL DEFAULT FALSE,
    quiz BOOLEAN NOT NULL DEFAULT FALSE,
    correct_option INTEGER,
    explanation TEXT,
    revision INTEGER NOT NULL DEFAULT 0,
    closed_at TEXT,
    created_at TEXT NOT NULL,
    CHECK (quiz = FALSE OR (correct_option IS NOT NULL AND multiple_choice = FALSE))
);

CREATE TABLE poll_options (
    message_id TEXT NOT NULL REFERENCES polls (message_id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    text TEXT NOT NULL,
    PRIMARY KEY (message_id, position)
);

CREATE TABLE poll_votes (
    message_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    voted_at TEXT NOT NULL,
    PRIMARY KEY (message_id, user_id, position),
    FOREIGN KEY (message_id, position) REFERENCES poll_options (message_id, position)
        ON DELETE CASCADE
);

-- The public voter list pages one option's voters, newest first.
CREATE INDEX poll_votes_option_idx ON poll_votes (message_id, position, voted_at DESC);
