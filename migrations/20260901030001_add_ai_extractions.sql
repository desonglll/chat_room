CREATE TABLE ai_extraction_runs (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    room_id TEXT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    client_request_id TEXT NOT NULL,
    from_at TEXT NOT NULL,
    to_at TEXT NOT NULL,
    model_option_id TEXT REFERENCES ai_model_options (id) ON DELETE SET NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    base_url TEXT NOT NULL DEFAULT '',
    api_key_env TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    message_count INTEGER,
    error_message TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    lease_expires_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (user_id, client_request_id),
    CHECK (from_at < to_at)
);

CREATE INDEX ai_extraction_runs_dispatch_idx
ON ai_extraction_runs (status, lease_expires_at, created_at);

CREATE INDEX ai_extraction_runs_user_room_idx
ON ai_extraction_runs (user_id, room_id, created_at DESC, id DESC);

CREATE TABLE ai_extraction_candidates (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    room_id TEXT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('decision', 'task')),
    title TEXT NOT NULL CHECK (LENGTH(title) BETWEEN 1 AND 120),
    detail TEXT NOT NULL CHECK (LENGTH(detail) <= 2000),
    inferred INTEGER NOT NULL DEFAULT 0,
    dedupe_key TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('proposed', 'confirmed', 'dismissed')),
    result_kind TEXT CHECK (result_kind IS NULL OR result_kind IN ('favorite', 'task')),
    result_id TEXT,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (user_id, room_id, dedupe_key)
);

CREATE INDEX ai_extraction_candidates_room_idx
ON ai_extraction_candidates (user_id, room_id, updated_at DESC, id DESC);

CREATE TABLE ai_extraction_candidate_sources (
    candidate_id TEXT NOT NULL REFERENCES ai_extraction_candidates (id) ON DELETE CASCADE,
    message_id TEXT NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (candidate_id, message_id),
    UNIQUE (candidate_id, ordinal)
);

CREATE TABLE ai_extraction_run_candidates (
    run_id TEXT NOT NULL REFERENCES ai_extraction_runs (id) ON DELETE CASCADE,
    candidate_id TEXT NOT NULL REFERENCES ai_extraction_candidates (id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (run_id, candidate_id),
    UNIQUE (run_id, ordinal)
);
