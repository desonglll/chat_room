-- TG-004: the columns the four chat types need (docs/tg/architecture.md §4.2).
-- Data is backfilled by 20261001000003_backfill_chat_types.sql.
--
-- `access_hash` carries a DEFAULT '' that architecture.md §4.2 does not show, because
-- ALTER TABLE ... ADD COLUMN ... NOT NULL requires a non-null default. Migration ..03
-- backfills every existing row and AppState::create_chat_with_owner generates one for every
-- new chat, so the empty string is unreachable in practice. A CHECK is deliberately not added:
-- SQLite cannot add one to an existing table without rebuilding it.
--
-- `linked_chat_id` must default to NULL — SQLite refuses ADD COLUMN with a REFERENCES clause
-- and any other default while foreign keys are enabled.
PRAGMA legacy_alter_table = OFF;

ALTER TABLE chats ADD COLUMN chat_type TEXT NOT NULL DEFAULT 'group'
    CHECK (chat_type IN ('private', 'group', 'supergroup', 'channel'));
ALTER TABLE chats ADD COLUMN username TEXT;
ALTER TABLE chats ADD COLUMN access_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE chats ADD COLUMN is_forum INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN linked_chat_id TEXT DEFAULT NULL
    REFERENCES chats (id) ON DELETE SET NULL;
ALTER TABLE chats ADD COLUMN slow_mode_seconds INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN auto_delete_seconds INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN signatures_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN history_visible_to_new_members INTEGER NOT NULL DEFAULT 1;
-- A projection, not the truth: the authoritative member count is COUNT(*) over chat_members.
-- A 200 000-member supergroup cannot be counted per request, so the column is maintained by
-- the same transaction that changes membership.
ALTER TABLE chats ADD COLUMN member_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE chats RENAME COLUMN name TO title;

-- The active-title uniqueness index has to be rebuilt around the new column name.
DROP INDEX chats_name_active_idx;
CREATE UNIQUE INDEX chats_title_active_idx ON chats (title) WHERE deleted_at IS NULL;

-- Public handles follow the same partial-uniqueness pattern as the title, with the extra
-- IS NOT NULL predicate because a chat without a public handle is the normal case.
CREATE UNIQUE INDEX chats_username_active_idx
ON chats (username) WHERE deleted_at IS NULL AND username IS NOT NULL;
