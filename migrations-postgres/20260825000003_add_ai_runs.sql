ALTER TABLE ai_thread_messages
ADD COLUMN status TEXT NOT NULL DEFAULT 'completed'
CHECK (status IN ('pending', 'streaming', 'completed', 'failed'));

ALTER TABLE ai_thread_messages
ADD COLUMN revision BIGINT NOT NULL DEFAULT 0;

ALTER TABLE ai_thread_messages
ADD COLUMN updated_at TIMESTAMPTZ;

UPDATE ai_thread_messages SET updated_at = created_at WHERE updated_at IS NULL;

ALTER TABLE ai_thread_messages ALTER COLUMN updated_at SET NOT NULL;

CREATE TABLE ai_runs (
    id UUID PRIMARY KEY NOT NULL,
    thread_id UUID NOT NULL REFERENCES ai_threads (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_message_id UUID NOT NULL REFERENCES ai_thread_messages (id) ON DELETE CASCADE,
    assistant_message_id UUID NOT NULL REFERENCES ai_thread_messages (id) ON DELETE CASCADE,
    client_request_id UUID NOT NULL,
    room_id UUID REFERENCES rooms (id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    context_message_count BIGINT,
    error_message TEXT,
    attempt_count BIGINT NOT NULL DEFAULT 0,
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE (user_id, client_request_id)
);

CREATE INDEX ai_runs_dispatch_idx
ON ai_runs (status, lease_expires_at, created_at);

CREATE UNIQUE INDEX ai_runs_one_active_per_thread_idx
ON ai_runs (thread_id) WHERE status IN ('queued', 'running');
