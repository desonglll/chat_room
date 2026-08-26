ALTER TABLE ai_thread_messages
ADD COLUMN stage TEXT NOT NULL DEFAULT 'completed'
CHECK (stage IN (
    'queued', 'preparing_context', 'retrieving_context', 'connecting_model',
    'waiting_for_model', 'reasoning', 'responding', 'completed', 'failed'
));

ALTER TABLE ai_thread_messages
ADD COLUMN stage_started_at TEXT;

UPDATE ai_thread_messages
SET stage = CASE status
        WHEN 'pending' THEN 'queued'
        WHEN 'streaming' THEN 'responding'
        ELSE status
    END,
    stage_started_at = COALESCE(updated_at, created_at);
