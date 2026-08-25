CREATE TABLE ai_model_options (
    id UUID PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic')),
    base_url TEXT NOT NULL,
    model TEXT NOT NULL,
    api_key_env TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

ALTER TABLE ai_runs ADD COLUMN model_option_id UUID REFERENCES ai_model_options(id) ON DELETE SET NULL;
ALTER TABLE ai_runs ADD COLUMN provider TEXT NOT NULL DEFAULT '';
ALTER TABLE ai_runs ADD COLUMN model TEXT NOT NULL DEFAULT '';
ALTER TABLE ai_runs ADD COLUMN base_url TEXT NOT NULL DEFAULT '';
ALTER TABLE ai_runs ADD COLUMN api_key_env TEXT NOT NULL DEFAULT '';
