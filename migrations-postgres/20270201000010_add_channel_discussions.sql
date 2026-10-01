-- TG-203: channel comments, carried by the channel's linked discussion group.
--
-- `chats.linked_chat_id` (20261001000002) links both ways: the channel points at its group and
-- the group back at its channel. Each post published while linked is copied into the group by
-- the database (every write path inserts messages itself — text, media, stickers, forwards,
-- scheduled sends — so the copy cannot be left to each path); comments are the group's reply
-- chain under that copy. The live poller delivers the copy to the group like any message.
CREATE TABLE channel_discussion_threads (
    channel_message_id UUID PRIMARY KEY NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    discussion_chat_id UUID NOT NULL REFERENCES chats (id) ON DELETE CASCADE,
    discussion_message_id UUID NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX channel_discussion_threads_chat_idx
    ON channel_discussion_threads (discussion_chat_id);

-- The copy: sent by "the channel" (no account), shown as forwarded from it. Only content kinds
-- that live entirely on the messages row (text, attachments, stickers) are copied as such; a
-- poll or a voice note arrives as its text/attachment.
CREATE FUNCTION messages_channel_discussion_copy() RETURNS TRIGGER AS $$
DECLARE
    channel RECORD;
    copy_id UUID := gen_random_uuid();
BEGIN
    IF NEW.sender_id IS NULL THEN
        RETURN NULL;
    END IF;
    SELECT id, title, linked_chat_id INTO channel FROM chats
    WHERE chats.id = NEW.room_id AND chats.chat_type = 'channel'
      AND chats.linked_chat_id IS NOT NULL AND chats.deleted_at IS NULL;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    INSERT INTO channel_discussion_threads
        (channel_message_id, channel_id, discussion_chat_id, discussion_message_id, created_at)
    VALUES (NEW.id, NEW.room_id, channel.linked_chat_id, copy_id, NEW.created_at);
    INSERT INTO messages
        (id, room_id, sender_id, sender, content, attachment_id, media_kind, sticker_id,
         forwarded_from_sender, forwarded_from_room_name, created_at)
    VALUES (copy_id, channel.linked_chat_id, NULL, channel.title, NEW.content, NEW.attachment_id,
            CASE WHEN NEW.media_kind = 'sticker' THEN 'sticker' ELSE NULL END,
            CASE WHEN NEW.media_kind = 'sticker' THEN NEW.sticker_id ELSE NULL END,
            channel.title, channel.title, NEW.created_at);
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_channel_discussion_copy AFTER INSERT ON messages
FOR EACH ROW EXECUTE FUNCTION messages_channel_discussion_copy();

-- Deleting a post (recall) takes its copy with it; the comments stay in the group as replies
-- to a recalled message, and the post's comment thread is no longer reachable from the channel.
CREATE FUNCTION messages_channel_discussion_recall() RETURNS TRIGGER AS $$
BEGIN
    UPDATE messages SET recalled_at = NEW.recalled_at
    WHERE recalled_at IS NULL AND id IN (
        SELECT discussion_message_id FROM channel_discussion_threads
        WHERE channel_message_id = NEW.id
    );
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_channel_discussion_recall AFTER UPDATE OF recalled_at ON messages
FOR EACH ROW WHEN (NEW.recalled_at IS NOT NULL AND OLD.recalled_at IS NULL)
EXECUTE FUNCTION messages_channel_discussion_recall();

-- A hard delete (auto-delete timer) recalls the copy before the mapping row cascades away.
CREATE FUNCTION messages_channel_discussion_delete() RETURNS TRIGGER AS $$
BEGIN
    UPDATE messages SET recalled_at = COALESCE(OLD.recalled_at, OLD.created_at)
    WHERE recalled_at IS NULL AND id IN (
        SELECT discussion_message_id FROM channel_discussion_threads
        WHERE channel_message_id = OLD.id
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_channel_discussion_delete BEFORE DELETE ON messages
FOR EACH ROW EXECUTE FUNCTION messages_channel_discussion_delete();

-- An edited post edits its copy.
CREATE FUNCTION messages_channel_discussion_edit() RETURNS TRIGGER AS $$
BEGIN
    UPDATE messages SET content = NEW.content, edited_at = NEW.edited_at
    WHERE id IN (
        SELECT discussion_message_id FROM channel_discussion_threads
        WHERE channel_message_id = NEW.id
    );
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER messages_channel_discussion_edit AFTER UPDATE OF content ON messages
FOR EACH ROW WHEN (NEW.content IS DISTINCT FROM OLD.content)
EXECUTE FUNCTION messages_channel_discussion_edit();
