-- room_participants (SQLite legacy table, see the paired migrations/ file of
-- the same number) was never carried over to the Postgres schema. This file
-- exists only to keep migration numbers in lockstep across both backends.
SELECT 1;
