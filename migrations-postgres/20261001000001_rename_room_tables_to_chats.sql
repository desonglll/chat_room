-- TG-004: the Room domain becomes the Chat domain (docs/tg/decisions.md D-003).
-- PostgreSQL twin of migrations/20261001000001_rename_room_tables_to_chats.sql.
--
-- Tables and indexes are renamed. Every `room_id` column keeps its name: D-003 freezes
-- `messages.room_id`, so `room_id` stays the single spelling of "foreign key to chats(id)"
-- across the schema. See docs/devlog/TG-004.md "Frozen interface".

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
-- dropped it in SQLite and the PostgreSQL bootstrap never created it.

-- Renaming a table leaves its indexes under their old names. Renaming the index that backs a
-- primary key or unique constraint renames that constraint with it. CHECK and foreign-key
-- constraint names are deliberately left alone: there are forty of them, they are never
-- referenced by name from application code, and renaming each one is risk without benefit.
ALTER INDEX rooms_pkey RENAME TO chats_pkey;
ALTER INDEX rooms_created_at_idx RENAME TO chats_created_at_idx;
ALTER INDEX rooms_name_active_idx RENAME TO chats_name_active_idx;

ALTER INDEX room_memberships_pkey RENAME TO chat_members_pkey;
ALTER INDEX room_memberships_user_idx RENAME TO chat_members_user_idx;
ALTER INDEX room_memberships_room_status_idx RENAME TO chat_members_room_status_idx;
ALTER INDEX idx_room_memberships_conversation_preferences
    RENAME TO chat_members_conversation_preferences_idx;

ALTER INDEX room_roles_pkey RENAME TO chat_roles_pkey;
ALTER INDEX room_roles_room_id_name_key RENAME TO chat_roles_room_id_name_key;

ALTER INDEX room_role_permissions_pkey RENAME TO chat_role_permissions_pkey;
ALTER INDEX room_permissions_pkey RENAME TO chat_permissions_pkey;

ALTER INDEX room_reads_pkey RENAME TO chat_reads_pkey;
ALTER INDEX room_reads_message_id_idx RENAME TO chat_reads_message_id_idx;

ALTER INDEX room_pins_pkey RENAME TO chat_pins_pkey;
ALTER INDEX room_pins_room_time_idx RENAME TO chat_pins_room_time_idx;

ALTER INDEX room_bans_pkey RENAME TO chat_bans_pkey;
ALTER INDEX room_bans_user_idx RENAME TO chat_bans_user_idx;

ALTER INDEX room_tasks_pkey RENAME TO chat_tasks_pkey;
ALTER INDEX room_tasks_room_status_idx RENAME TO chat_tasks_room_status_idx;
ALTER INDEX room_tasks_assignee_idx RENAME TO chat_tasks_assignee_idx;

ALTER INDEX room_ai_policies_pkey RENAME TO chat_ai_policies_pkey;

-- PostgreSQL does NOT rewrite plpgsql bodies when a table is renamed: the body is text that
-- is resolved at execution time. record_room_join_notification() is the only function in the
-- schema whose source references a renamed table (checked against pg_proc.prosrc on a fully
-- migrated database), and without this the next join request would fail at runtime while the
-- migration itself reported success. The function keeps its name, and so do the two triggers
-- that call it, because they are named after the notification kind `room_join_request` —
-- a client-visible wire value frozen until M6.
CREATE OR REPLACE FUNCTION record_room_join_notification() RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO notifications (
        id, recipient_id, kind, actor_id, room_id, dedupe_key, created_at
    )
    SELECT 'room_join_request:' || NEW.room_id::text || ':' || NEW.user_id::text || ':' ||
               manager.user_id::text || ':' || NEW.requested_at::text,
           manager.user_id, 'room_join_request', NEW.user_id, NEW.room_id,
           'room_join_request:' || NEW.room_id::text || ':' || NEW.user_id::text || ':' ||
               manager.user_id::text || ':' || NEW.requested_at::text,
           NEW.requested_at
    FROM chat_members AS manager
    JOIN chat_role_permissions AS permission ON permission.role_id = manager.role_id
    WHERE manager.room_id = NEW.room_id AND manager.status = 'active'
      AND manager.user_id <> NEW.user_id AND permission.permission_key = 'members.review'
    ON CONFLICT(dedupe_key) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
