-- TG-501 chat folders (Telegram's filters). The server stores the rules; clients evaluate which
-- chats belong to a folder from their conversation list (`packages/core/src/domain/chatFolders.ts`).
-- `include_types` is a JSON array of `private` / `groups` / `channels`.
CREATE TABLE chat_folders (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    emoji TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL,
    include_types TEXT NOT NULL DEFAULT '[]',
    exclude_muted BOOLEAN NOT NULL DEFAULT FALSE,
    exclude_read BOOLEAN NOT NULL DEFAULT FALSE,
    exclude_archived BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX chat_folders_user_idx ON chat_folders (user_id, position);

CREATE TABLE chat_folder_chats (
    folder_id TEXT NOT NULL REFERENCES chat_folders (id) ON DELETE CASCADE,
    room_id TEXT NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK (mode IN ('include', 'exclude')),
    PRIMARY KEY (folder_id, room_id)
);
