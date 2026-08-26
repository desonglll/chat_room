ALTER TABLE ai_thread_messages
ADD COLUMN trace TEXT NOT NULL DEFAULT '[]';
