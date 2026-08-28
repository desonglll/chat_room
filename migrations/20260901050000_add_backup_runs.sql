CREATE TABLE backup_runs (
    id TEXT PRIMARY KEY NOT NULL,
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('manual', 'scheduled')),
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    database_kind TEXT NOT NULL CHECK (database_kind IN ('sqlite', 'postgres')),
    target_backend TEXT NOT NULL,
    includes_files INTEGER NOT NULL DEFAULT 0,
    artifact_path TEXT,
    artifact_sha256 TEXT,
    artifact_size_bytes INTEGER,
    manifest_created_at TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    duration_ms INTEGER,
    error TEXT
);

CREATE INDEX backup_runs_recent_idx ON backup_runs (started_at DESC, id DESC);
