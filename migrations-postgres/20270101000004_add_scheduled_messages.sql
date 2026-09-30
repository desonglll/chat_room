-- TG-404: scheduled and silent messages.
--
-- A scheduled message lives in its own table until delivery and is INSERTed into `messages`
-- only when it is delivered. Nothing that reads `messages` (history, search, unread counts,
-- chat-list previews, notifications, the TG-502 unarchive trigger) can therefore see it early,
-- without a single query having to learn a new filter. On delivery the row is deleted and the
-- message is inserted with the same id, in one transaction: a second deliverer (another
-- instance, or «send now» racing the scheduler) finds nothing to delete and inserts nothing.
CREATE TABLE scheduled_messages (
    id UUID PRIMARY KEY NOT NULL,
    room_id UUID NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    sender_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    entities TEXT NOT NULL DEFAULT '[]',
    reply_to_id UUID REFERENCES messages (id) ON DELETE SET NULL,
    silent BOOLEAN NOT NULL DEFAULT FALSE,
    scheduled_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX scheduled_messages_due_idx ON scheduled_messages (scheduled_at);
CREATE INDEX scheduled_messages_owner_idx
    ON scheduled_messages (room_id, sender_id, scheduled_at);

-- A silent message is a normal message that produces no notification (and so no Web Push:
-- push jobs are created only from `notifications` rows).
ALTER TABLE messages ADD COLUMN silent BOOLEAN NOT NULL DEFAULT FALSE;

DROP TRIGGER IF EXISTS notifications_reply_insert ON messages;
CREATE TRIGGER notifications_reply_insert
AFTER INSERT ON messages FOR EACH ROW
WHEN (NEW.reply_to_id IS NOT NULL AND NOT NEW.silent)
EXECUTE FUNCTION record_reply_notification();

CREATE OR REPLACE FUNCTION record_mention_notification() RETURNS TRIGGER AS $$
DECLARE
    event_key TEXT;
BEGIN
    event_key := 'mention:' || NEW.message_id::text || ':' || NEW.mentioned_user_id::text;
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, message_id, dedupe_key, created_at
    )
    SELECT event_key, NEW.mentioned_user_id, 'mention', messages.sender_id,
           messages.room_id, messages.id, event_key, NEW.created_at
    FROM messages
    WHERE messages.id = NEW.message_id
      AND messages.sender_id IS NOT NULL
      AND messages.sender_id <> NEW.mentioned_user_id
      AND NOT messages.silent
    ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
