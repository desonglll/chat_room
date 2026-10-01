-- TG-407: location messages. A location is an ordinary message (`media_kind = 'location'`) with
-- one row here. A live location keeps only its latest point (no trail is stored, so nothing can
-- be tracked after the fact) and accepts updates only until `live_until`; stopping early moves
-- `live_until` to the stop time. NULL `live_until` = a static location.
CREATE TABLE message_locations (
    message_id TEXT PRIMARY KEY NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    accuracy_m REAL,
    heading INTEGER,
    title TEXT NOT NULL DEFAULT '',
    address TEXT NOT NULL DEFAULT '',
    live_until TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX message_locations_live_idx ON message_locations (live_until)
    WHERE live_until IS NOT NULL;
