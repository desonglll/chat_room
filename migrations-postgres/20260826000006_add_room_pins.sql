ALTER TABLE messages
ADD COLUMN favorite_id UUID REFERENCES favorites (id) ON DELETE SET NULL;

CREATE INDEX messages_favorite_id_idx ON messages (favorite_id);

CREATE TABLE room_pins (
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    pinned_by UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    pinned_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (room_id, message_id)
);

CREATE INDEX room_pins_room_time_idx
ON room_pins (room_id, pinned_at DESC, message_id);

INSERT INTO room_permissions (permission_key, description)
VALUES ('message.pin', 'Pin and unpin room messages')
ON CONFLICT (permission_key) DO NOTHING;

INSERT INTO room_role_permissions (role_id, permission_key)
SELECT id, 'message.pin' FROM room_roles WHERE name IN ('owner', 'admin')
ON CONFLICT (role_id, permission_key) DO NOTHING;
