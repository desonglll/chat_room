-- TG-207 slow mode looks up a member's newest message in a chat on every live send in a
-- slow-mode chat (`SELECT MAX(created_at) ... WHERE room_id = ? AND sender_id = ?`). Without
-- this index that is a scan of the chat's history.
CREATE INDEX messages_room_sender_created_idx ON messages (room_id, sender_id, created_at);
