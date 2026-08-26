-- A destination-specific queue prevents Graphiti and Qdrant consumers from racing.
CREATE TABLE message_graph_outbox (
    message_id UUID PRIMARY KEY NOT NULL,
    room_id UUID NOT NULL,
    operation TEXT NOT NULL CHECK (operation IN ('upsert', 'delete')),
    attempt_count BIGINT NOT NULL DEFAULT 0,
    generation BIGINT NOT NULL DEFAULT 1,
    next_attempt_at TIMESTAMPTZ NOT NULL,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX message_graph_outbox_ready_idx
ON message_graph_outbox (next_attempt_at, updated_at);

CREATE OR REPLACE FUNCTION enqueue_message_graph_change()
RETURNS TRIGGER AS $$
DECLARE
    target_id UUID;
    target_room_id UUID;
    target_operation TEXT;
BEGIN
    target_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END;
    target_room_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.room_id ELSE NEW.room_id END;
    target_operation := CASE
        WHEN TG_OP = 'DELETE' THEN 'delete'
        WHEN NEW.recalled_at IS NULL AND btrim(NEW.content) <> '' THEN 'upsert'
        ELSE 'delete'
    END;
    INSERT INTO message_graph_outbox
        (message_id, room_id, operation, next_attempt_at, updated_at)
    VALUES (target_id, target_room_id, target_operation, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET room_id = EXCLUDED.room_id,
        operation = EXCLUDED.operation, attempt_count = 0,
        generation = message_graph_outbox.generation + 1,
        next_attempt_at = EXCLUDED.next_attempt_at,
        last_error = NULL, updated_at = EXCLUDED.updated_at;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_graph_after_insert
AFTER INSERT ON messages FOR EACH ROW EXECUTE FUNCTION enqueue_message_graph_change();

CREATE TRIGGER messages_graph_after_update
AFTER UPDATE OF content, recalled_at ON messages
FOR EACH ROW EXECUTE FUNCTION enqueue_message_graph_change();

CREATE TRIGGER messages_graph_after_delete
AFTER DELETE ON messages FOR EACH ROW EXECUTE FUNCTION enqueue_message_graph_change();

INSERT INTO message_graph_outbox
    (message_id, room_id, operation, next_attempt_at, updated_at)
SELECT messages.id, messages.room_id, 'upsert', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
FROM messages
JOIN rooms ON rooms.id = messages.room_id
WHERE messages.recalled_at IS NULL
  AND btrim(messages.content) <> ''
  AND rooms.deleted_at IS NULL;
