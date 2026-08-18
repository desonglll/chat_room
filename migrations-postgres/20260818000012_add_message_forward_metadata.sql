-- Point-in-time snapshot of a forwarded message's original sender/room. No FK:
-- a forward must survive the source message/room being recalled or deleted later.
ALTER TABLE messages ADD COLUMN forwarded_from_sender TEXT;
ALTER TABLE messages ADD COLUMN forwarded_from_room_name TEXT;
