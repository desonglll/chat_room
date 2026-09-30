-- TG-405: per-chat auto-delete ("自毁计时器"). `chats.auto_delete_seconds` (TG-004) is the
-- chat's current timer; each message remembers ITS deadline in `messages.auto_delete_at`,
-- stamped when it is inserted, so changing the timer only affects messages sent afterwards
-- (Telegram). A trigger stamps it for every insert path (text, media, stickers, polls, voice,
-- albums, forwards, scheduled delivery …) without each learning a new column.
ALTER TABLE messages ADD COLUMN auto_delete_at TIMESTAMPTZ;

CREATE INDEX messages_auto_delete_idx ON messages (auto_delete_at)
    WHERE auto_delete_at IS NOT NULL;

CREATE FUNCTION stamp_message_auto_delete() RETURNS TRIGGER AS $$
DECLARE
    seconds INTEGER;
BEGIN
    IF NEW.auto_delete_at IS NULL THEN
        SELECT auto_delete_seconds INTO seconds FROM chats WHERE id = NEW.room_id;
        IF seconds IS NOT NULL AND seconds > 0 THEN
            NEW.auto_delete_at := NEW.created_at + make_interval(secs => seconds);
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_stamp_auto_delete
BEFORE INSERT ON messages
FOR EACH ROW EXECUTE FUNCTION stamp_message_auto_delete();
