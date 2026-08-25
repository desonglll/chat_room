ALTER TABLE ai_thread_messages
ADD COLUMN sources TEXT NOT NULL DEFAULT '[]';
