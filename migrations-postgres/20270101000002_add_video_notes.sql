-- TG-402: round video messages ("video notes").
--
-- A video note is an attachment message with `messages.media_kind = 'video_note'` plus one
-- `video_notes` row keyed by the message, exactly like TG-401's voice notes: the message row
-- stays the source of truth for chat, sender, order, recall and history, and this table only
-- adds the playback projection beside it.
--
-- `thumbnail` is an optional small JPEG (the recorder's first frame, <= 16 KiB) shown before
-- the video loads and when autoplay is off. `duration_source` records whether `duration_ms`
-- came from the container ('container') or from the recorder ('client').
--
-- `video_note_listens` is the watched state, one row per viewer. The sender never has a row
-- for their own message: for the sender, "watched" means "any row exists".
CREATE TABLE video_notes (
    message_id UUID PRIMARY KEY REFERENCES messages (id) ON DELETE CASCADE,
    duration_ms INTEGER NOT NULL CHECK (duration_ms > 0),
    thumbnail BYTEA,
    duration_source TEXT NOT NULL CHECK (duration_source IN ('container', 'client')),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE video_note_listens (
    message_id UUID NOT NULL REFERENCES video_notes (message_id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    listened_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (message_id, user_id)
);
