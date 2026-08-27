CREATE TABLE ai_usage_records_next (
    id TEXT PRIMARY KEY NOT NULL,
    admission_id TEXT NOT NULL UNIQUE,
    user_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    room_id TEXT,
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

INSERT INTO ai_usage_records_next
    (id, admission_id, user_id, room_id, feature, model_option_id, provider, model, status,
     token_source, input_tokens, output_tokens, total_tokens, duration_ms,
     estimated_cost_micros, created_at)
SELECT id, admission_id, user_id, room_id, feature, model_option_id, provider, model, status,
       token_source, input_tokens, output_tokens, total_tokens, duration_ms,
       estimated_cost_micros, created_at
FROM ai_usage_records;

DROP TABLE ai_usage_records;
ALTER TABLE ai_usage_records_next RENAME TO ai_usage_records;

CREATE INDEX ai_usage_records_created_idx ON ai_usage_records (created_at);
CREATE INDEX ai_usage_records_room_idx ON ai_usage_records (room_id, created_at);
CREATE INDEX ai_usage_records_model_idx ON ai_usage_records (model_option_id, created_at);
