CREATE TABLE backup_runs (
    id UUID PRIMARY KEY,
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('manual', 'scheduled')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    database_kind TEXT NOT NULL CHECK (database_kind IN ('sqlite', 'postgres')),
    target_backend TEXT NOT NULL,
    includes_files BOOLEAN NOT NULL DEFAULT FALSE,
    artifact_path TEXT,
    artifact_sha256 TEXT,
    artifact_size_bytes BIGINT,
    manifest_created_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,
    error TEXT
);

CREATE INDEX backup_runs_recent_idx ON backup_runs (started_at DESC, id DESC);
