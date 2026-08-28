CREATE TABLE attachment_visual_projections (
    attachment_id TEXT NOT NULL REFERENCES attachments (id) ON DELETE CASCADE,
    room_id TEXT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    prompt_version INTEGER NOT NULL CHECK (prompt_version > 0),
    projection TEXT NOT NULL,
    search_text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (attachment_id, model, prompt_version)
);

CREATE INDEX attachment_visual_projections_room_idx
ON attachment_visual_projections (room_id, updated_at DESC);

CREATE TRIGGER attachments_visual_index_after_sensitivity_update
AFTER UPDATE OF is_sensitive ON attachments
WHEN OLD.is_sensitive <> NEW.is_sensitive
BEGIN
    INSERT INTO message_index_outbox
        (message_id, operation, next_attempt_at, updated_at)
    SELECT messages.id,
           CASE
               WHEN messages.recalled_at IS NULL
                    AND (
                        trim(messages.content) <> ''
                        OR (
                            NEW.is_sensitive = 0
                            AND EXISTS (
                                SELECT 1 FROM attachment_visual_projections projections
                                WHERE projections.attachment_id = NEW.id
                                  AND projections.room_id = messages.room_id
                            )
                        )
                    )
               THEN 'upsert'
               ELSE 'delete'
           END,
           CURRENT_TIMESTAMP,
           CURRENT_TIMESTAMP
    FROM messages
    WHERE messages.attachment_id = NEW.id
    ON CONFLICT (message_id) DO UPDATE SET
        operation = excluded.operation,
        attempt_count = 0,
        generation = message_index_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL,
        updated_at = excluded.updated_at;
END;
