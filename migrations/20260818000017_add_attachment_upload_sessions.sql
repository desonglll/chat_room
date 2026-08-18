-- Resumable/chunked upload sessions. Tracked in the DB (not just the staging
-- file) so we can authorize/list in-progress uploads without trusting the
-- filesystem alone.
CREATE TABLE attachment_uploads (
    id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    uploader_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    declared_size_bytes INTEGER NOT NULL CHECK (declared_size_bytes > 0),
    received_bytes INTEGER NOT NULL DEFAULT 0,
    fingerprint TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'in_progress' CHECK (status IN ('in_progress', 'completed', 'aborted')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX attachment_uploads_uploader_idx ON attachment_uploads (uploader_id, room_id, status);
