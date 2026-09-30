-- TG-004: the columns the four chat types need (docs/tg/architecture.md §4.2).
-- PostgreSQL twin of migrations/20261001000002_add_chat_type_and_channel_fields.sql.
-- Data is backfilled by 20261001000003_backfill_chat_types.sql.
--
-- `access_hash` carries a DEFAULT '' that architecture.md §4.2 does not show, so that the
-- column can be added NOT NULL to a populated table in the same way as on SQLite. Migration
-- ..03 backfills every existing row and AppState::create_chat_with_owner generates one for
-- every new chat.
--
-- The three boolean flags are BOOLEAN here and INTEGER on SQLite, which is the repository's
-- existing convention for the two adapters and what sqlx decodes to `bool` on both.

ALTER TABLE chats ADD COLUMN chat_type TEXT NOT NULL DEFAULT 'group'
    CHECK (chat_type IN ('private', 'group', 'supergroup', 'channel'));
ALTER TABLE chats ADD COLUMN username TEXT;
ALTER TABLE chats ADD COLUMN access_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE chats ADD COLUMN is_forum BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE chats ADD COLUMN linked_chat_id UUID DEFAULT NULL
    REFERENCES chats (id) ON DELETE SET NULL;
ALTER TABLE chats ADD COLUMN slow_mode_seconds INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN auto_delete_seconds INTEGER NOT NULL DEFAULT 0;
ALTER TABLE chats ADD COLUMN signatures_enabled BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE chats ADD COLUMN history_visible_to_new_members BOOLEAN NOT NULL DEFAULT TRUE;
-- A projection, not the truth: the authoritative member count is COUNT(*) over chat_members.
-- A 200 000-member supergroup cannot be counted per request, so the column is maintained by
-- the same transaction that changes membership.
ALTER TABLE chats ADD COLUMN member_count INTEGER NOT NULL DEFAULT 0;

ALTER TABLE chats RENAME COLUMN name TO title;

-- PostgreSQL keeps a partial index working across a column rename, but the index name has to
-- follow the column it is named after.
ALTER INDEX chats_name_active_idx RENAME TO chats_title_active_idx;

-- Public handles follow the same partial-uniqueness pattern as the title, with the extra
-- IS NOT NULL predicate because a chat without a public handle is the normal case.
CREATE UNIQUE INDEX chats_username_active_idx
ON chats (username) WHERE deleted_at IS NULL AND username IS NOT NULL;
