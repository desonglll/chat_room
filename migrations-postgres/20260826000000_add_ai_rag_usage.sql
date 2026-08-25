ALTER TABLE ai_thread_messages
ADD COLUMN retrieved_message_count BIGINT;

ALTER TABLE ai_runs
ADD COLUMN retrieved_message_count BIGINT;
