-- TG-401: voice messages.
--
-- A voice message is an attachment message with `messages.media_kind = 'voice'` (TG-302's
-- column) plus one `voice_notes` row keyed by the message: the message row stays the source
-- of truth for chat, sender, order, recall and history, and this table only adds the
-- playback projection beside it.
--
-- `waveform` is Telegram's encoding: 100 samples of 5 bits each, packed little-endian into
-- 63 bytes. The samples are computed by the recorder from the live PCM and validated by the
-- server (docs/devlog/TG-401.md, Decisions). `duration_source` records whether
-- `duration_ms` came from the container ('container') or, when the container carries no
-- usable duration, from the recorder ('client').
--
-- `voice_listens` is the listened ("played") state, one row per listener. The sender never
-- has a row for their own message: for the sender, "listened" means "any row exists".
CREATE TABLE voice_notes (
    message_id TEXT PRIMARY KEY REFERENCES messages (id) ON DELETE CASCADE,
    duration_ms INTEGER NOT NULL CHECK (duration_ms > 0),
    waveform BLOB NOT NULL,
    duration_source TEXT NOT NULL CHECK (duration_source IN ('container', 'client')),
    created_at TEXT NOT NULL
);

CREATE TABLE voice_listens (
    message_id TEXT NOT NULL REFERENCES voice_notes (message_id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    listened_at TEXT NOT NULL,
    PRIMARY KEY (message_id, user_id)
);
