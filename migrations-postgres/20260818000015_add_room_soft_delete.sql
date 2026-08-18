-- Soft-delete rooms instead of hard-deleting them: messages/attachments stay
-- intact, the room just disappears from listings. Replace the plain UNIQUE(name)
-- with a partial unique index so a deleted room's name can be reused.
ALTER TABLE rooms DROP CONSTRAINT rooms_name_key;
ALTER TABLE rooms ADD COLUMN deleted_at TIMESTAMPTZ;
CREATE UNIQUE INDEX rooms_name_active_idx ON rooms (name) WHERE deleted_at IS NULL;
