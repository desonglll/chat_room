CREATE TABLE ai_model_options (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    api_key_env TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE ai_runs ADD COLUMN model_option_id TEXT REFERENCES ai_model_options(id) ON DELETE SET NULL;
ALTER TABLE ai_runs ADD COLUMN provider TEXT NOT NULL DEFAULT '';
ALTER TABLE ai_runs ADD COLUMN model TEXT NOT NULL DEFAULT '';
ALTER TABLE ai_runs ADD COLUMN base_url TEXT NOT NULL DEFAULT '';
ALTER TABLE ai_runs ADD COLUMN api_key_env TEXT NOT NULL DEFAULT '';
