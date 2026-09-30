-- TG-202: channel broadcast semantics.
--
-- views_count is a projection of message_views (one row per message and viewer, so a view is
-- counted once per account). The server aggregates view increments per chat over a short
-- window and writes both in one transaction (src/chats/channel_views.rs).
ALTER TABLE messages ADD COLUMN views_count INTEGER NOT NULL DEFAULT 0;

-- The author signature of a channel post, frozen when the post is written while the
-- channel's signatures are on. NULL = unsigned (every non-channel message).
ALTER TABLE messages ADD COLUMN post_author TEXT;

CREATE TABLE message_views (
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    viewed_at TEXT NOT NULL,
    PRIMARY KEY (message_id, user_id)
);

CREATE INDEX message_views_user_idx ON message_views (user_id);

-- Every write path inserts messages itself (text, attachments, stickers, polls, forwards,
-- favorites, and later voice/GIF/scheduled sends), so the signature is set by the database
-- on insert rather than by each path.
CREATE TRIGGER messages_channel_post_author AFTER INSERT ON messages
WHEN NEW.sender_id IS NOT NULL AND EXISTS (
    SELECT 1 FROM chats
    WHERE chats.id = NEW.room_id AND chats.chat_type = 'channel'
      AND chats.signatures_enabled <> 0
)
BEGIN
    UPDATE messages SET post_author = (
        SELECT COALESCE(NULLIF(chat_members.nickname, ''), NULLIF(users.display_name, ''),
                        users.username)
        FROM users
        LEFT JOIN chat_members ON chat_members.room_id = NEW.room_id
          AND chat_members.user_id = users.id
        WHERE users.id = NEW.sender_id
    ) WHERE id = NEW.id;
END;
