-- Migration 008 concatenated BLOB UUIDs with text suffixes. SQLite retained the
-- TEXT storage class but the resulting bytes were not valid UTF-8, so decoding a
-- role id during join requests failed. Normalize every system role to stable ASCII.
PRAGMA defer_foreign_keys = ON;

UPDATE room_memberships
SET role_id = lower(hex(room_id)) || ':' || COALESCE(
    (SELECT name FROM room_roles WHERE room_roles.id = room_memberships.role_id),
    'member'
);

UPDATE room_role_permissions
SET role_id = (
    SELECT lower(hex(room_roles.room_id)) || ':' || room_roles.name
    FROM room_roles
    WHERE room_roles.id = room_role_permissions.role_id
);

UPDATE room_roles
SET id = lower(hex(room_id)) || ':' || name;
