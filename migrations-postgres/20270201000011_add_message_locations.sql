-- TG-407: location messages. A location is an ordinary message (`media_kind = 'location'`) with
-- one row here. A live location keeps only its latest point (no trail is stored, so nothing can
-- be tracked after the fact) and accepts updates only until `live_until`; stopping early moves
-- `live_until` to the stop time. NULL `live_until` = a static location.
CREATE TABLE message_locations (
    message_id UUID PRIMARY KEY NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    accuracy_m DOUBLE PRECISION,
    heading INTEGER,
    title TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL DEFAULT '',
    live_until TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX message_locations_live_idx ON message_locations (live_until)
    WHERE live_until IS NOT NULL;
