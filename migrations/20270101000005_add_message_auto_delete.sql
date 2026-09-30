-- TG-405: per-chat auto-delete ("自毁计时器"). `chats.auto_delete_seconds` (TG-004) is the
-- chat's current timer; each message remembers ITS deadline in `messages.auto_delete_at`,
-- stamped when it is inserted, so changing the timer only affects messages sent afterwards
-- (Telegram). A trigger stamps it for every insert path (text, media, stickers, polls, voice,
-- albums, forwards, scheduled delivery …) without each learning a new column.
ALTER TABLE messages ADD COLUMN auto_delete_at TEXT;

CREATE INDEX messages_auto_delete_idx ON messages (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL;

CREATE TRIGGER messages_stamp_auto_delete
AFTER INSERT ON messages
WHEN NEW.auto_delete_at IS NULL
 AND (SELECT auto_delete_seconds FROM chats WHERE id = NEW.room_id) > 0
BEGIN
    UPDATE messages
    SET auto_delete_at = strftime(
        '%Y-%m-%dT%H:%M:%f+00:00',
        NEW.created_at,
        '+' || (SELECT auto_delete_seconds FROM chats WHERE id = NEW.room_id) || ' seconds'
    )
    WHERE id = NEW.id;
END;
