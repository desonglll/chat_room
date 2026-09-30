-- TG-008: cloud drafts — one composer draft per (chat, account), synced across devices.
--
-- The FK column is `room_id`, not `chat_id`: TG-004's frozen-non-rename-1
-- (docs/devlog/TG-004.md) keeps `room_id` as the single spelling of "foreign key to
-- chats(id)" across the whole schema until a dedicated rename task changes all of them
-- at once.
--
-- `topic_id` has no foreign key on purpose: forum topics land in M2 (TG-204); the column
-- exists now so the frozen `draft_updated` frame (docs/devlog/TG-007.md §3) never has to
-- change shape. M2 owns adding a constraint if it wants one.
CREATE TABLE chat_drafts (
    room_id UUID NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    reply_to_message_id UUID REFERENCES messages (id) ON DELETE SET NULL,
    topic_id UUID,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (room_id, user_id)
);

-- The M1 chat list previews an account's drafts across chats, newest first.
CREATE INDEX chat_drafts_user_idx ON chat_drafts (user_id, updated_at DESC);
