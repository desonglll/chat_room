CREATE TABLE notifications (
    id TEXT PRIMARY KEY NOT NULL,
    recipient_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'friend_request', 'room_join_request', 'mention', 'reply', 'ai_run_completed'
    )),
    actor_id UUID REFERENCES users (id) ON DELETE SET NULL,
    room_id UUID REFERENCES rooms (id) ON DELETE SET NULL,
    message_id UUID REFERENCES messages (id) ON DELETE SET NULL,
    run_id UUID REFERENCES ai_runs (id) ON DELETE SET NULL,
    summary TEXT NOT NULL DEFAULT '',
    dedupe_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    read_at TIMESTAMPTZ
);

CREATE INDEX notifications_recipient_cursor_idx
    ON notifications (recipient_id, created_at DESC, id DESC);

CREATE INDEX notifications_recipient_unread_idx
    ON notifications (recipient_id, created_at DESC)
    WHERE read_at IS NULL;

CREATE FUNCTION record_friend_request_notification() RETURNS TRIGGER AS $$
DECLARE
    target_id UUID;
    event_key TEXT;
BEGIN
    target_id := CASE WHEN NEW.requested_by_id = NEW.user_low_id
        THEN NEW.user_high_id ELSE NEW.user_low_id END;
    event_key := 'friend_request:' || NEW.requested_by_id::text || ':' || NEW.created_at::text;
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, dedupe_key, created_at
    ) VALUES (
        event_key, target_id, 'friend_request', NEW.requested_by_id, event_key, NEW.created_at
    ) ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER notifications_friend_request_insert
AFTER INSERT ON friendships FOR EACH ROW
WHEN (NEW.status = 'pending')
EXECUTE FUNCTION record_friend_request_notification();

CREATE FUNCTION record_room_join_notification() RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, dedupe_key, created_at
    )
    SELECT 'room_join_request:' || NEW.room_id::text || ':' || NEW.user_id::text || ':' ||
               manager.user_id::text || ':' || NEW.requested_at::text,
           manager.user_id, 'room_join_request', NEW.user_id, NEW.room_id,
           'room_join_request:' || NEW.room_id::text || ':' || NEW.user_id::text || ':' ||
               manager.user_id::text || ':' || NEW.requested_at::text,
           NEW.requested_at
    FROM room_memberships AS manager
    JOIN room_role_permissions AS permission ON permission.role_id = manager.role_id
    WHERE manager.room_id = NEW.room_id AND manager.status = 'active'
      AND manager.user_id <> NEW.user_id AND permission.permission_key = 'members.review'
    ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER notifications_room_join_insert
AFTER INSERT ON room_memberships FOR EACH ROW
WHEN (NEW.status = 'pending')
EXECUTE FUNCTION record_room_join_notification();

CREATE TRIGGER notifications_room_join_update
AFTER UPDATE OF status ON room_memberships FOR EACH ROW
WHEN (NEW.status = 'pending' AND OLD.status <> 'pending')
EXECUTE FUNCTION record_room_join_notification();

CREATE FUNCTION record_mention_notification() RETURNS TRIGGER AS $$
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
    ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER notifications_mention_insert
AFTER INSERT ON message_mentions FOR EACH ROW
EXECUTE FUNCTION record_mention_notification();

CREATE FUNCTION record_reply_notification() RETURNS TRIGGER AS $$
DECLARE
    event_key TEXT;
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, message_id, dedupe_key, created_at
    )
    SELECT 'reply:' || NEW.id::text || ':' || source.sender_id::text,
           source.sender_id, 'reply', NEW.sender_id, NEW.room_id, NEW.id,
           'reply:' || NEW.id::text || ':' || source.sender_id::text, NEW.created_at
    FROM messages AS source
    WHERE source.id = NEW.reply_to_id AND source.sender_id IS NOT NULL
      AND source.sender_id <> NEW.sender_id
    ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER notifications_reply_insert
AFTER INSERT ON messages FOR EACH ROW
WHEN (NEW.reply_to_id IS NOT NULL)
EXECUTE FUNCTION record_reply_notification();

CREATE FUNCTION record_ai_run_notification() RETURNS TRIGGER AS $$
DECLARE
    event_key TEXT;
BEGIN
    event_key := 'ai_run_completed:' || NEW.id::text;
    INSERT INTO notifications (
        id, recipient_id, kind, room_id, run_id, dedupe_key, created_at
    ) VALUES (
        event_key, NEW.user_id, 'ai_run_completed', NEW.room_id, NEW.id, event_key, NEW.updated_at
    ) ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER notifications_ai_run_complete
AFTER UPDATE OF status ON ai_runs FOR EACH ROW
WHEN (NEW.status = 'completed' AND OLD.status <> 'completed')
EXECUTE FUNCTION record_ai_run_notification();
