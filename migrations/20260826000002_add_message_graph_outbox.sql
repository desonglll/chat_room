-- A destination-specific queue prevents Graphiti and Qdrant consumers from racing.
CREATE TABLE message_graph_outbox (
    message_id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    generation INTEGER NOT NULL DEFAULT 1,
    next_attempt_at TEXT NOT NULL,
    last_error TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX message_graph_outbox_ready_idx
ON message_graph_outbox (next_attempt_at, updated_at);

CREATE TRIGGER messages_graph_after_insert
AFTER INSERT ON messages
BEGIN
    INSERT INTO message_graph_outbox
        (message_id, room_id, operation, next_attempt_at, updated_at)
    VALUES
        (NEW.id, NEW.room_id,
         CASE WHEN NEW.recalled_at IS NULL AND trim(NEW.content) <> '' THEN 'upsert' ELSE 'delete' END,
         CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET room_id = excluded.room_id,
        operation = excluded.operation, attempt_count = 0,
        generation = message_graph_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL, updated_at = excluded.updated_at;
END;

CREATE TRIGGER messages_graph_after_update
AFTER UPDATE OF content, recalled_at ON messages
BEGIN
    INSERT INTO message_graph_outbox
        (message_id, room_id, operation, next_attempt_at, updated_at)
    VALUES
        (NEW.id, NEW.room_id,
         CASE WHEN NEW.recalled_at IS NULL AND trim(NEW.content) <> '' THEN 'upsert' ELSE 'delete' END,
         CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET room_id = excluded.room_id,
        operation = excluded.operation, attempt_count = 0,
        generation = message_graph_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL, updated_at = excluded.updated_at;
END;

CREATE TRIGGER messages_graph_after_delete
AFTER DELETE ON messages
BEGIN
    INSERT INTO message_graph_outbox
        (message_id, room_id, operation, next_attempt_at, updated_at)
    VALUES (OLD.id, OLD.room_id, 'delete', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET room_id = excluded.room_id,
        operation = 'delete', attempt_count = 0,
        generation = message_graph_outbox.generation + 1,
        next_attempt_at = excluded.next_attempt_at,
        last_error = NULL, updated_at = excluded.updated_at;
END;

INSERT INTO message_graph_outbox
    (message_id, room_id, operation, next_attempt_at, updated_at)
SELECT messages.id, messages.room_id, 'upsert', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
FROM messages
JOIN rooms ON rooms.id = messages.room_id
WHERE messages.recalled_at IS NULL
  AND trim(messages.content) <> ''
  AND rooms.deleted_at IS NULL;
