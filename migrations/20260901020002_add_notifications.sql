CREATE TABLE notifications (
    id TEXT PRIMARY KEY NOT NULL,
    recipient_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'friend_request', 'room_join_request', 'mention', 'reply', 'ai_run_completed'
    )),
    actor_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    room_id TEXT REFERENCES rooms (id) ON DELETE SET NULL,
    message_id TEXT REFERENCES messages (id) ON DELETE SET NULL,
    run_id TEXT REFERENCES ai_runs (id) ON DELETE SET NULL,
    summary TEXT NOT NULL DEFAULT '',
    dedupe_key TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    read_at TEXT
);

CREATE INDEX notifications_recipient_cursor_idx
    ON notifications (recipient_id, created_at DESC, id DESC);

CREATE INDEX notifications_recipient_unread_idx
    ON notifications (recipient_id, created_at DESC)
    WHERE read_at IS NULL;

CREATE TRIGGER notifications_friend_request_insert
AFTER INSERT ON friendships
WHEN NEW.status = 'pending'
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, dedupe_key, created_at
    )
    SELECT 'friend_request:' || lower(hex(NEW.requested_by_id)) || ':' || NEW.created_at,
           CASE WHEN NEW.requested_by_id = NEW.user_low_id
                THEN NEW.user_high_id ELSE NEW.user_low_id END,
           'friend_request', NEW.requested_by_id,
           'friend_request:' || lower(hex(NEW.requested_by_id)) || ':' || NEW.created_at,
           NEW.created_at
    ON CONFLICT(dedupe_key) DO NOTHING;
END;

CREATE TRIGGER notifications_room_join_insert
AFTER INSERT ON room_memberships
WHEN NEW.status = 'pending'
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, dedupe_key, created_at
    )
    SELECT 'room_join_request:' || lower(hex(NEW.room_id)) || ':' ||
               lower(hex(NEW.user_id)) || ':' || lower(hex(manager.user_id)) || ':' ||
               NEW.requested_at,
           manager.user_id, 'room_join_request', NEW.user_id, NEW.room_id,
           'room_join_request:' || lower(hex(NEW.room_id)) || ':' ||
               lower(hex(NEW.user_id)) || ':' || lower(hex(manager.user_id)) || ':' ||
               NEW.requested_at,
           NEW.requested_at
    FROM room_memberships AS manager
    JOIN room_role_permissions AS permission ON permission.role_id = manager.role_id
    WHERE manager.room_id = NEW.room_id AND manager.status = 'active'
      AND manager.user_id <> NEW.user_id AND permission.permission_key = 'members.review'
    ON CONFLICT(dedupe_key) DO NOTHING;
END;

CREATE TRIGGER notifications_room_join_update
AFTER UPDATE OF status ON room_memberships
WHEN NEW.status = 'pending' AND OLD.status <> 'pending'
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, dedupe_key, created_at
    )
    SELECT 'room_join_request:' || lower(hex(NEW.room_id)) || ':' ||
               lower(hex(NEW.user_id)) || ':' || lower(hex(manager.user_id)) || ':' ||
               NEW.requested_at,
           manager.user_id, 'room_join_request', NEW.user_id, NEW.room_id,
           'room_join_request:' || lower(hex(NEW.room_id)) || ':' ||
               lower(hex(NEW.user_id)) || ':' || lower(hex(manager.user_id)) || ':' ||
               NEW.requested_at,
           NEW.requested_at
    FROM room_memberships AS manager
    JOIN room_role_permissions AS permission ON permission.role_id = manager.role_id
    WHERE manager.room_id = NEW.room_id AND manager.status = 'active'
      AND manager.user_id <> NEW.user_id AND permission.permission_key = 'members.review'
    ON CONFLICT(dedupe_key) DO NOTHING;
END;

CREATE TRIGGER notifications_mention_insert
AFTER INSERT ON message_mentions
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, message_id, dedupe_key, created_at
    )
    SELECT 'mention:' || lower(hex(NEW.message_id)) || ':' ||
               lower(hex(NEW.mentioned_user_id)),
           NEW.mentioned_user_id, 'mention', messages.sender_id, messages.room_id,
           messages.id,
           'mention:' || lower(hex(NEW.message_id)) || ':' ||
               lower(hex(NEW.mentioned_user_id)),
           NEW.created_at
    FROM messages
    WHERE messages.id = NEW.message_id
      AND messages.sender_id IS NOT NULL
      AND messages.sender_id <> NEW.mentioned_user_id
    ON CONFLICT(dedupe_key) DO NOTHING;
END;

CREATE TRIGGER notifications_reply_insert
AFTER INSERT ON messages
WHEN NEW.reply_to_id IS NOT NULL
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, message_id, dedupe_key, created_at
    )
    SELECT 'reply:' || lower(hex(NEW.id)) || ':' || lower(hex(source.sender_id)),
           source.sender_id, 'reply', NEW.sender_id, NEW.room_id, NEW.id,
           'reply:' || lower(hex(NEW.id)) || ':' || lower(hex(source.sender_id)),
           NEW.created_at
    FROM messages AS source
    WHERE source.id = NEW.reply_to_id AND source.sender_id IS NOT NULL
      AND source.sender_id <> NEW.sender_id
    ON CONFLICT(dedupe_key) DO NOTHING;
END;

CREATE TRIGGER notifications_ai_run_complete
AFTER UPDATE OF status ON ai_runs
WHEN NEW.status = 'completed' AND OLD.status <> 'completed'
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, room_id, run_id, dedupe_key, created_at
    ) VALUES (
        'ai_run_completed:' || lower(hex(NEW.id)), NEW.user_id, 'ai_run_completed',
        NEW.room_id, NEW.id, 'ai_run_completed:' || lower(hex(NEW.id)), NEW.updated_at
    ) ON CONFLICT(dedupe_key) DO NOTHING;
END;
