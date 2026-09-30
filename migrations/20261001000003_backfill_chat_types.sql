-- TG-004: give every pre-existing chat a type, an access hash and a member count.
--
-- docs/tg/architecture.md §4.1: direct_conversations.room_id was already a foreign key to
-- rooms(id) and every message already lived in one `messages` table keyed by room_id. A
-- one-to-one chat was therefore already a chat row; direct_conversations is only the side
-- table that finds it from two user ids. That is why classifying the four chat types costs a
-- single UPDATE and no data movement.
PRAGMA legacy_alter_table = OFF;

UPDATE chats
SET chat_type = CASE
        WHEN EXISTS (
            SELECT 1 FROM direct_conversations
            WHERE direct_conversations.room_id = chats.id
        ) THEN 'private'
        ELSE 'group'
    END;

-- 64 bits of randomness, so a public handle can be resolved without leaking the row id.
-- hex(randomblob(8)) is SQLite's only CSPRNG-backed source of bytes.
UPDATE chats
SET access_hash = lower(hex(randomblob(8)))
WHERE access_hash = '';

UPDATE chats
SET member_count = (
        SELECT COUNT(*) FROM chat_members
        WHERE chat_members.room_id = chats.id AND chat_members.status = 'active'
    );
