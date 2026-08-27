ALTER TABLE sessions ADD COLUMN management_id TEXT;
ALTER TABLE sessions ADD COLUMN device_name TEXT NOT NULL DEFAULT 'Unknown device';
ALTER TABLE sessions ADD COLUMN ip_hint TEXT NOT NULL DEFAULT '';
ALTER TABLE sessions ADD COLUMN last_used_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;

UPDATE sessions
SET management_id = md5(id::text || created_at::text || random()::text),
    last_used_at = created_at;

CREATE UNIQUE INDEX sessions_management_id_idx
    ON sessions (management_id)
    WHERE management_id IS NOT NULL;
CREATE INDEX sessions_user_last_used_idx
    ON sessions (user_id, last_used_at DESC, id);
