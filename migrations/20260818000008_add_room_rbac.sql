ALTER TABLE rooms
ADD COLUMN creator_user_id TEXT REFERENCES users (id) ON DELETE SET NULL;

ALTER TABLE rooms
ADD COLUMN join_policy TEXT NOT NULL DEFAULT 'open'
CHECK (join_policy IN ('open', 'approval'));

CREATE TABLE room_permissions (
    permission_key TEXT PRIMARY KEY NOT NULL,
    description TEXT NOT NULL
);

INSERT INTO room_permissions (permission_key, description) VALUES
    ('message.send', 'Send messages in the room'),
    ('message.edit_own', 'Edit messages sent by this account'),
    ('message.recall_own', 'Recall messages sent by this account'),
    ('room.settings', 'Rename the room and change its access settings'),
    ('room.delete', 'Delete the room'),
    ('members.review', 'Approve or reject membership requests'),
    ('members.invite', 'Invite accounts to the room'),
    ('members.remove', 'Remove active members'),
    ('members.roles', 'Change member roles');

CREATE TABLE room_roles (
    id TEXT PRIMARY KEY NOT NULL,
    room_id TEXT NOT NULL,
    name TEXT NOT NULL,
    is_system INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    UNIQUE (room_id, name),
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE
);

CREATE TABLE room_role_permissions (
    role_id TEXT NOT NULL,
    permission_key TEXT NOT NULL,
    PRIMARY KEY (role_id, permission_key),
    FOREIGN KEY (role_id) REFERENCES room_roles (id) ON DELETE CASCADE,
    FOREIGN KEY (permission_key) REFERENCES room_permissions (permission_key) ON DELETE CASCADE
);

CREATE TABLE room_memberships (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'invited', 'active')),
    invited_by TEXT,
    requested_at TEXT NOT NULL,
    joined_at TEXT,
    PRIMARY KEY (room_id, user_id),
    FOREIGN KEY (room_id) REFERENCES rooms (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (role_id) REFERENCES room_roles (id) ON DELETE CASCADE,
    FOREIGN KEY (invited_by) REFERENCES users (id) ON DELETE SET NULL
);

CREATE INDEX room_memberships_user_idx ON room_memberships (user_id, status);
CREATE INDEX room_memberships_room_status_idx ON room_memberships (room_id, status);

INSERT INTO room_roles (id, room_id, name, is_system, created_at)
SELECT id || ':owner', id, 'owner', 1, created_at FROM rooms;
INSERT INTO room_roles (id, room_id, name, is_system, created_at)
SELECT id || ':admin', id, 'admin', 1, created_at FROM rooms;
INSERT INTO room_roles (id, room_id, name, is_system, created_at)
SELECT id || ':member', id, 'member', 1, created_at FROM rooms;

INSERT INTO room_role_permissions (role_id, permission_key)
SELECT room_roles.id, room_permissions.permission_key
FROM room_roles CROSS JOIN room_permissions
WHERE room_roles.name = 'owner';
INSERT INTO room_role_permissions (role_id, permission_key)
SELECT room_roles.id, room_permissions.permission_key
FROM room_roles CROSS JOIN room_permissions
WHERE room_roles.name = 'admin' AND room_permissions.permission_key <> 'room.delete'
    AND room_permissions.permission_key <> 'members.roles';
INSERT INTO room_role_permissions (role_id, permission_key)
SELECT room_roles.id, room_permissions.permission_key
FROM room_roles CROSS JOIN room_permissions
WHERE room_roles.name = 'member' AND room_permissions.permission_key IN
    ('message.send', 'message.edit_own', 'message.recall_own');

INSERT INTO room_memberships
    (room_id, user_id, role_id, status, requested_at, joined_at)
SELECT participants.room_id, participants.user_id,
       CASE WHEN participants.joined_at = (
           SELECT MIN(first_join.joined_at) FROM room_participants AS first_join
           WHERE first_join.room_id = participants.room_id
       ) THEN participants.room_id || ':owner' ELSE participants.room_id || ':member' END,
       'active', participants.joined_at, participants.joined_at
FROM room_participants AS participants;

UPDATE rooms SET creator_user_id = (
    SELECT memberships.user_id FROM room_memberships AS memberships
    JOIN room_roles ON room_roles.id = memberships.role_id
    WHERE memberships.room_id = rooms.id AND room_roles.name = 'owner'
    ORDER BY memberships.joined_at, memberships.user_id LIMIT 1
);
