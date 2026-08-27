CREATE TABLE audit_events (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL CHECK (scope IN ('system', 'room')),
    room_id TEXT,
    actor_user_id TEXT NOT NULL,
    actor_username TEXT NOT NULL,
    event_type TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    details_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    CHECK ((scope = 'system' AND room_id IS NULL) OR (scope = 'room' AND room_id IS NOT NULL))
);

CREATE TABLE room_bans (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    banned_by TEXT,
    banned_at TEXT NOT NULL,
    PRIMARY KEY (room_id, user_id),
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (banned_by) REFERENCES users (id) ON DELETE SET NULL
);

CREATE INDEX room_bans_user_idx ON room_bans (user_id, banned_at DESC);

CREATE INDEX audit_events_scope_cursor_idx ON audit_events (scope, created_at DESC, id DESC);
CREATE INDEX audit_events_room_cursor_idx ON audit_events (room_id, created_at DESC, id DESC);
CREATE INDEX audit_events_actor_idx ON audit_events (actor_user_id, created_at DESC);
CREATE INDEX audit_events_type_idx ON audit_events (event_type, created_at DESC);

CREATE TRIGGER audit_events_reject_update
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit events are append-only');
END;

CREATE TRIGGER audit_events_reject_delete
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit events are append-only');
END;
