ALTER TABLE ai_thread_messages
ADD COLUMN status TEXT NOT NULL DEFAULT 'completed'
CHECK (status IN ('pending', 'streaming', 'completed', 'failed'));

ALTER TABLE ai_thread_messages
ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;

ALTER TABLE ai_thread_messages
ADD COLUMN updated_at TEXT;

UPDATE ai_thread_messages SET updated_at = created_at WHERE updated_at IS NULL;

CREATE TABLE ai_runs (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL REFERENCES ai_threads (id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    user_message_id TEXT NOT NULL REFERENCES ai_thread_messages (id) ON DELETE CASCADE,
    assistant_message_id TEXT NOT NULL REFERENCES ai_thread_messages (id) ON DELETE CASCADE,
    client_request_id TEXT NOT NULL,
    room_id TEXT REFERENCES rooms (id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    context_message_count INTEGER,
    error_message TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    lease_expires_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (user_id, client_request_id)
);

CREATE INDEX ai_runs_dispatch_idx
ON ai_runs (status, lease_expires_at, created_at);

CREATE UNIQUE INDEX ai_runs_one_active_per_thread_idx
ON ai_runs (thread_id) WHERE status IN ('queued', 'running');
