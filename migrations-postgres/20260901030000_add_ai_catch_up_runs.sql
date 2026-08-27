ALTER TABLE ai_runs
ADD COLUMN purpose TEXT NOT NULL DEFAULT 'question'
CHECK (purpose IN ('question', 'catch_up'));

ALTER TABLE ai_runs
ADD COLUMN source_after_message_id UUID;

ALTER TABLE ai_runs
ADD COLUMN source_through_message_id UUID;

ALTER TABLE ai_runs
ADD COLUMN source_message_count BIGINT;
