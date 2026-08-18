CREATE TABLE users (
    id UUID PRIMARY KEY,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    avatar_emoji TEXT NOT NULL DEFAULT '',
    display_name TEXT NOT NULL DEFAULT '',
    signature TEXT NOT NULL DEFAULT '',
    homepage TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL
);
CREATE UNIQUE INDEX users_username_ci_idx ON users (LOWER(username));

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);

CREATE TABLE rooms (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL DEFAULT '',
    creator_user_id UUID REFERENCES users (id) ON DELETE SET NULL,
    join_policy TEXT NOT NULL DEFAULT 'open' CHECK (join_policy IN ('open', 'approval')),
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX rooms_created_at_idx ON rooms (created_at, id);

CREATE TABLE attachments (
    id UUID PRIMARY KEY,
    access_key UUID NOT NULL UNIQUE,
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    uploader_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes > 0),
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX attachments_room_id_idx ON attachments (room_id, created_at, id);

CREATE TABLE messages (
    id UUID PRIMARY KEY,
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    sender_id UUID REFERENCES users (id) ON DELETE SET NULL,
    sender TEXT NOT NULL,
    content TEXT NOT NULL,
    attachment_id UUID REFERENCES attachments (id) ON DELETE SET NULL,
    reply_to_id UUID REFERENCES messages (id) ON DELETE SET NULL,
    recalled_at TIMESTAMPTZ,
    edited_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX messages_room_cursor_idx ON messages (room_id, created_at, id);
CREATE INDEX messages_attachment_id_idx ON messages (attachment_id);
CREATE INDEX messages_reply_to_id_idx ON messages (reply_to_id);
CREATE INDEX messages_recalled_at_idx ON messages (room_id, recalled_at);
CREATE INDEX messages_edited_at_idx ON messages (room_id, edited_at);

CREATE TABLE room_reads (
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    read_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (room_id, user_id)
);
CREATE INDEX room_reads_message_id_idx ON room_reads (message_id);

CREATE TABLE room_permissions (
    permission_key TEXT PRIMARY KEY,
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
    id TEXT PRIMARY KEY,
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    is_system BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (room_id, name)
);

CREATE TABLE room_role_permissions (
    role_id TEXT NOT NULL REFERENCES room_roles (id) ON DELETE CASCADE,
    permission_key TEXT NOT NULL REFERENCES room_permissions (permission_key) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_key)
);

CREATE TABLE room_memberships (
    room_id UUID NOT NULL REFERENCES rooms (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role_id TEXT NOT NULL REFERENCES room_roles (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'invited', 'active')),
    invited_by UUID REFERENCES users (id) ON DELETE SET NULL,
    requested_at TIMESTAMPTZ NOT NULL,
    joined_at TIMESTAMPTZ,
    PRIMARY KEY (room_id, user_id)
);
CREATE INDEX room_memberships_user_idx ON room_memberships (user_id, status);
CREATE INDEX room_memberships_room_status_idx ON room_memberships (room_id, status);
