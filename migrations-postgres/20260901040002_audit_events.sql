CREATE TABLE audit_events (
    id UUID PRIMARY KEY,
    scope TEXT NOT NULL CHECK (scope IN ('system', 'room')),
    room_id UUID,
    actor_user_id UUID NOT NULL,
    actor_username TEXT NOT NULL,
    event_type TEXT NOT NULL,
    target_type TEXT,
    target_id TEXT,
    details_json TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL,
    CHECK ((scope = 'system' AND room_id IS NULL) OR (scope = 'room' AND room_id IS NOT NULL))
);

CREATE TABLE room_bans (
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    banned_by UUID REFERENCES users (id) ON DELETE SET NULL,
    banned_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (room_id, user_id)
);

CREATE INDEX room_bans_user_idx ON room_bans (user_id, banned_at DESC);

CREATE INDEX audit_events_scope_cursor_idx ON audit_events (scope, created_at DESC, id DESC);
CREATE INDEX audit_events_room_cursor_idx ON audit_events (room_id, created_at DESC, id DESC);
CREATE INDEX audit_events_actor_idx ON audit_events (actor_user_id, created_at DESC);
CREATE INDEX audit_events_type_idx ON audit_events (event_type, created_at DESC);

CREATE FUNCTION reject_audit_event_change() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit events are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_events_reject_update
BEFORE UPDATE ON audit_events
FOR EACH ROW EXECUTE FUNCTION reject_audit_event_change();

CREATE TRIGGER audit_events_reject_delete
BEFORE DELETE ON audit_events
FOR EACH ROW EXECUTE FUNCTION reject_audit_event_change();
