ALTER TABLE room_memberships ADD COLUMN is_pinned BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE room_memberships ADD COLUMN is_archived BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE room_memberships ADD COLUMN notification_level TEXT NOT NULL DEFAULT 'all'
    CHECK (notification_level IN ('all', 'mentions', 'none'));
ALTER TABLE room_memberships ADD COLUMN muted_until TIMESTAMPTZ;
ALTER TABLE room_memberships ADD COLUMN preferences_updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;

CREATE INDEX idx_room_memberships_conversation_preferences
    ON room_memberships (user_id, status, is_archived, is_pinned);
