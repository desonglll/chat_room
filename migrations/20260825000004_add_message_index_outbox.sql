CREATE TABLE message_index_outbox (
    message_id TEXT PRIMARY KEY NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    generation INTEGER NOT NULL DEFAULT 1,
    next_attempt_at TEXT NOT NULL,
    last_error TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX message_index_outbox_ready_idx
ON message_index_outbox (next_attempt_at, updated_at);

CREATE TRIGGER messages_index_after_insert
AFTER INSERT ON messages
BEGIN
    INSERT INTO message_index_outbox (message_id, operation, next_attempt_at, updated_at)
    VALUES (NEW.id, CASE WHEN NEW.recalled_at IS NULL AND trim(NEW.content) <> '' THEN 'upsert' ELSE 'delete' END,
            CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET operation = excluded.operation,
        attempt_count = 0, generation = message_index_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL, updated_at = excluded.updated_at;
END;

CREATE TRIGGER messages_index_after_update
AFTER UPDATE OF content, recalled_at ON messages
BEGIN
    INSERT INTO message_index_outbox (message_id, operation, next_attempt_at, updated_at)
    VALUES (NEW.id, CASE WHEN NEW.recalled_at IS NULL AND trim(NEW.content) <> '' THEN 'upsert' ELSE 'delete' END,
            CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET operation = excluded.operation,
        attempt_count = 0, generation = message_index_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL, updated_at = excluded.updated_at;
END;

CREATE TRIGGER messages_index_after_delete
AFTER DELETE ON messages
BEGIN
    INSERT INTO message_index_outbox (message_id, operation, next_attempt_at, updated_at)
    VALUES (OLD.id, 'delete', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET operation = 'delete',
        attempt_count = 0, generation = message_index_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL, updated_at = excluded.updated_at;
END;

INSERT INTO message_index_outbox (message_id, operation, next_attempt_at, updated_at)
SELECT id, 'upsert', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
FROM messages WHERE recalled_at IS NULL AND trim(content) <> '';
