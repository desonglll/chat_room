CREATE TABLE system_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO system_settings (key, value) VALUES ('chat_rooms_locked', 'false');
