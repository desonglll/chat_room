-- TG-508: notification defaults per chat type and per-chat exceptions.
--
-- The existing per-chat `chat_members.notification_level` / `muted_until` stay the mute switch
-- (1 h / 8 h / 2 d = `muted_until`; forever = level `none`). These tables add what Telegram's
-- «Notifications and Sounds» adds: a default per chat type (private / groups / channels) and a
-- per-chat exception that can override the default's on/off, message preview and sound. NULL in
-- an exception column means "inherit the default". Precedence lives in one pure function
-- (`src/notifications/exceptions.rs::decide`).
CREATE TABLE notification_defaults (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('private', 'group', 'channel')),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    preview BOOLEAN NOT NULL DEFAULT TRUE,
    sound TEXT NOT NULL DEFAULT 'default',
    PRIMARY KEY (user_id, scope)
);

CREATE TABLE notification_exceptions (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    enabled BOOLEAN,
    preview BOOLEAN,
    sound TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, room_id)
);
