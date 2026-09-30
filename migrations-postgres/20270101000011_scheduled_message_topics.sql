-- TG-204 × TG-404: a scheduled message remembers the forum topic it was scheduled into, so it
-- is delivered there (and re-checked against a closed topic at delivery). NULL = General, as
-- for `messages.topic_id`. Deleting a topic deletes its pending scheduled messages with it.
ALTER TABLE scheduled_messages ADD COLUMN topic_id UUID REFERENCES forum_topics (id) ON DELETE CASCADE;
