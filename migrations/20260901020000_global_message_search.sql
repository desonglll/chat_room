CREATE INDEX messages_visible_search_idx
    ON messages (room_id, recalled_at, created_at DESC, id DESC);
