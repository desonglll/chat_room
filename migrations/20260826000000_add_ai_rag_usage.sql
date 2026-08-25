ALTER TABLE ai_thread_messages
ADD COLUMN retrieved_message_count INTEGER;

ALTER TABLE ai_runs
ADD COLUMN retrieved_message_count INTEGER;
