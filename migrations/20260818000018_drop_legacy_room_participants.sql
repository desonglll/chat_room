-- room_participants was superseded by room_memberships in the RBAC rewrite
-- (20260818000008_add_room_rbac.sql) and has had no reader or writer since;
-- the Postgres schema never carried it forward. Drop it here so the two
-- backends converge on the same table set.
DROP INDEX IF EXISTS room_participants_user_id_idx;
DROP TABLE IF EXISTS room_participants;
