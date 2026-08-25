CREATE OR REPLACE FUNCTION enqueue_message_index_change()
RETURNS TRIGGER AS $$
DECLARE
    target_id UUID;
    target_operation TEXT;
BEGIN
    target_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END;
    target_operation := CASE
        WHEN TG_OP = 'DELETE' THEN 'delete'
        WHEN NEW.recalled_at IS NULL AND btrim(NEW.content) <> '' THEN 'upsert'
        ELSE 'delete'
    END;
    INSERT INTO message_index_outbox (message_id, operation, next_attempt_at, updated_at)
    VALUES (target_id, target_operation, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT (message_id) DO UPDATE SET operation = EXCLUDED.operation,
        attempt_count = 0, generation = message_index_outbox.generation + 1,
        next_attempt_at = EXCLUDED.next_attempt_at,
        last_error = NULL, updated_at = EXCLUDED.updated_at;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
