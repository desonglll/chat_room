-- TG-004: the Room domain becomes the Chat domain (docs/tg/decisions.md D-003).
--
-- Tables and indexes are renamed. Every `room_id` column keeps its name: D-003 freezes
-- `messages.room_id`, so `room_id` stays the single spelling of "foreign key to chats(id)"
-- across the schema. See docs/devlog/TG-004.md "Frozen interface".
--
-- `legacy_alter_table` is forced OFF rather than assumed. Measured on SQLite 3.51.0: with it
-- ON (which is what the sqlite3 CLI ships), ALTER TABLE ... RENAME TO reports success but
-- leaves `REFERENCES rooms` clauses in other tables and `FROM room_memberships` inside the
-- bodies of the two notifications_room_join_* triggers pointing at names that no longer
-- exist; the breakage only surfaces at the next schema reparse. With the pragma OFF SQLite
-- rewrites both. The other documented precondition, foreign_keys = ON, is set by
-- src/storage.rs. tests/chat_rename_schema_test.rs proves the result rather than trusting
-- this comment: flipping this line to ON makes it fail.
PRAGMA legacy_alter_table = OFF;

ALTER TABLE rooms RENAME TO chats;
ALTER TABLE room_memberships RENAME TO chat_members;
ALTER TABLE room_roles RENAME TO chat_roles;
ALTER TABLE room_role_permissions RENAME TO chat_role_permissions;
ALTER TABLE room_permissions RENAME TO chat_permissions;
ALTER TABLE room_reads RENAME TO chat_reads;
ALTER TABLE room_pins RENAME TO chat_pins;
ALTER TABLE room_bans RENAME TO chat_bans;
ALTER TABLE room_tasks RENAME TO chat_tasks;
-- Not on the task card's list, but the tenth surviving member of the family: its primary key
-- is a foreign key to rooms(id). Added by 20260901030002_add_ai_governance.sql.
ALTER TABLE room_ai_policies RENAME TO chat_ai_policies;

-- room_participants is deliberately absent: 20260818000018_drop_legacy_room_participants.sql
-- already dropped it in both adapters.

-- SQLite has no RENAME INDEX, and a table rename leaves index names untouched.
DROP INDEX rooms_created_at_idx;
CREATE INDEX chats_created_at_idx ON chats (created_at, id);

DROP INDEX rooms_name_active_idx;
CREATE UNIQUE INDEX chats_name_active_idx ON chats (name) WHERE deleted_at IS NULL;

DROP INDEX room_memberships_user_idx;
CREATE INDEX chat_members_user_idx ON chat_members (user_id, status);

DROP INDEX room_memberships_room_status_idx;
CREATE INDEX chat_members_room_status_idx ON chat_members (room_id, status);

DROP INDEX idx_room_memberships_conversation_preferences;
CREATE INDEX chat_members_conversation_preferences_idx
ON chat_members (user_id, status, is_archived, is_pinned);

DROP INDEX room_reads_message_id_idx;
CREATE INDEX chat_reads_message_id_idx ON chat_reads (message_id);

DROP INDEX room_pins_room_time_idx;
CREATE INDEX chat_pins_room_time_idx ON chat_pins (room_id, pinned_at DESC, message_id);

DROP INDEX room_bans_user_idx;
CREATE INDEX chat_bans_user_idx ON chat_bans (user_id, banned_at DESC);

DROP INDEX room_tasks_room_status_idx;
CREATE INDEX chat_tasks_room_status_idx
ON chat_tasks (room_id, status, updated_at DESC, id DESC);

DROP INDEX room_tasks_assignee_idx;
CREATE INDEX chat_tasks_assignee_idx ON chat_tasks (assignee_id, status, due_at);

-- The triggers notifications_room_join_insert / _update keep their names: they are named
-- after the notification kind `room_join_request`, which clients consume and which stays
-- frozen until M6. SQLite already rewrote the table names inside their bodies.
