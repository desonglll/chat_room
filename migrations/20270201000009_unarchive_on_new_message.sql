-- TG-502: Telegram's archive rule. A new message in a chat pulls it out of every member's
-- archive, except for the sender and for members who have the chat muted when it arrives
-- (notifications off, or a `muted_until` still in the future). A muted archived chat stays
-- archived. Enforced here rather than in each send path so that every insert path (text,
-- forward, sticker, poll, attachment, favorites) obeys it without a second implementation.
-- julianday() compares instants whatever text format `muted_until` was written in.
CREATE TRIGGER chat_members_unarchive_on_message
AFTER INSERT ON messages
BEGIN
    UPDATE chat_members
    SET is_archived = FALSE, preferences_updated_at = CURRENT_TIMESTAMP
    WHERE room_id = NEW.room_id
      AND is_archived
      AND status = 'active'
      AND (NEW.sender_id IS NULL OR user_id <> NEW.sender_id)
      AND notification_level <> 'none'
      AND (muted_until IS NULL OR julianday(muted_until) <= julianday('now'));
END;
