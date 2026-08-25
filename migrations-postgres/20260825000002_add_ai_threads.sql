CREATE TABLE ai_threads (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    room_id UUID REFERENCES rooms (id) ON DELETE SET NULL,
    thinking_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX ai_threads_user_updated_idx
ON ai_threads (user_id, updated_at DESC);

CREATE TABLE ai_thread_messages (
    id UUID PRIMARY KEY,
    thread_id UUID NOT NULL REFERENCES ai_threads (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    room_id UUID REFERENCES rooms (id) ON DELETE SET NULL,
    context_message_count BIGINT,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX ai_thread_messages_thread_created_idx
ON ai_thread_messages (thread_id, created_at ASC);
