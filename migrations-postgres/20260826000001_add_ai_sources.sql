ALTER TABLE ai_thread_messages
ADD COLUMN sources JSONB NOT NULL DEFAULT '[]'::JSONB;
