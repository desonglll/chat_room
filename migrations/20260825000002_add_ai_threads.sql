CREATE TABLE ai_threads (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    room_id TEXT REFERENCES rooms (id) ON DELETE SET NULL,
    thinking_enabled INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX ai_threads_user_updated_idx
ON ai_threads (user_id, updated_at DESC);

CREATE TABLE ai_thread_messages (
    id TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NOT NULL REFERENCES ai_threads (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    room_id TEXT REFERENCES rooms (id) ON DELETE SET NULL,
    context_message_count INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX ai_thread_messages_thread_created_idx
ON ai_thread_messages (thread_id, created_at ASC);
