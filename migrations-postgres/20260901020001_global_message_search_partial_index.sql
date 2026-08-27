DROP INDEX messages_visible_search_idx;

CREATE INDEX messages_visible_search_idx
    ON messages (room_id, created_at DESC, id DESC)
    WHERE recalled_at IS NULL;
