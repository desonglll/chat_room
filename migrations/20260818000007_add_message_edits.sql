ALTER TABLE messages
ADD COLUMN edited_at TEXT;

CREATE INDEX messages_edited_at_idx ON messages (room_id, edited_at);
