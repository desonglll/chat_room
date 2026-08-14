CREATE TABLE rooms (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE INDEX rooms_created_at_idx ON rooms (created_at, id);

CREATE TABLE app_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
