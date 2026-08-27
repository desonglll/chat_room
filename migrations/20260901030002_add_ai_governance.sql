CREATE TABLE room_ai_policies (
    room_id TEXT PRIMARY KEY NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK (mode IN ('disabled', 'members', 'admins')),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    updated_by TEXT REFERENCES users (id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE ai_governance_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    max_concurrent_runs INTEGER NOT NULL DEFAULT 8 CHECK (max_concurrent_runs > 0),
    daily_user_token_limit INTEGER CHECK (daily_user_token_limit IS NULL OR daily_user_token_limit > 0),
    daily_room_token_limit INTEGER CHECK (daily_room_token_limit IS NULL OR daily_room_token_limit > 0),
    allowlist_enabled INTEGER NOT NULL DEFAULT 0,
    admission_revision INTEGER NOT NULL DEFAULT 0,
    updated_by TEXT REFERENCES users (id) ON DELETE SET NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO ai_governance_settings (id, updated_at) VALUES (1, CURRENT_TIMESTAMP);

CREATE TABLE ai_governance_models (
    model_option_id TEXT PRIMARY KEY NOT NULL,
    allowed INTEGER NOT NULL DEFAULT 1,
    input_price_micros_per_million INTEGER NOT NULL DEFAULT 0
        CHECK (input_price_micros_per_million >= 0),
    output_price_micros_per_million INTEGER NOT NULL DEFAULT 0
        CHECK (output_price_micros_per_million >= 0),
    updated_at TEXT NOT NULL
);

CREATE TABLE ai_admissions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    room_id TEXT REFERENCES rooms (id) ON DELETE CASCADE,
    feature TEXT NOT NULL CHECK (feature IN ('suggestion', 'question', 'catch_up', 'extraction')),
    model_option_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    reserved_tokens INTEGER NOT NULL DEFAULT 0 CHECK (reserved_tokens >= 0),
    input_price_micros_per_million INTEGER NOT NULL DEFAULT 0,
    output_price_micros_per_million INTEGER NOT NULL DEFAULT 0,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX ai_admissions_active_idx ON ai_admissions (expires_at, created_at);
CREATE INDEX ai_admissions_user_idx ON ai_admissions (user_id, expires_at);
CREATE INDEX ai_admissions_room_idx ON ai_admissions (room_id, expires_at);

CREATE TABLE ai_usage_records (
    id TEXT PRIMARY KEY NOT NULL,
    admission_id TEXT NOT NULL UNIQUE,
    user_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    room_id TEXT REFERENCES rooms (id) ON DELETE SET NULL,
    feature TEXT NOT NULL,
    model_option_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
    token_source TEXT NOT NULL DEFAULT 'estimated' CHECK (token_source = 'estimated'),
    input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    total_tokens INTEGER NOT NULL DEFAULT 0 CHECK (total_tokens >= 0),
    duration_ms INTEGER NOT NULL DEFAULT 0 CHECK (duration_ms >= 0),
    estimated_cost_micros INTEGER NOT NULL DEFAULT 0 CHECK (estimated_cost_micros >= 0),
    created_at TEXT NOT NULL
);

CREATE INDEX ai_usage_records_created_idx ON ai_usage_records (created_at);
CREATE INDEX ai_usage_records_room_idx ON ai_usage_records (room_id, created_at);
CREATE INDEX ai_usage_records_model_idx ON ai_usage_records (model_option_id, created_at);

ALTER TABLE ai_runs ADD COLUMN admission_id TEXT REFERENCES ai_admissions (id) ON DELETE SET NULL;
ALTER TABLE ai_extraction_runs ADD COLUMN admission_id TEXT REFERENCES ai_admissions (id) ON DELETE SET NULL;
