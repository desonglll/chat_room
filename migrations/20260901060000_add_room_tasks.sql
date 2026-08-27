CREATE TABLE room_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (LENGTH(title) BETWEEN 1 AND 120),
    status TEXT NOT NULL CHECK (status IN ('open', 'in_progress', 'done', 'cancelled')),
    assignee_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    created_by_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    created_by_name TEXT NOT NULL,
    source_message_id TEXT REFERENCES messages (id) ON DELETE SET NULL,
    due_at TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX room_tasks_room_status_idx
ON room_tasks (room_id, status, updated_at DESC, id DESC);

CREATE INDEX room_tasks_assignee_idx
ON room_tasks (assignee_id, status, due_at);
