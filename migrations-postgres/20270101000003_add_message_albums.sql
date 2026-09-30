-- TG-403: albums (media groups).
--
-- An album is 2-10 ordinary media messages that share one `grouped_id`, inserted in one
-- transaction by `src/messages/albums`. Every member stays a full message (own id, own
-- attachment, own reactions, own recall), so history, search, unread counts, previews and
-- forwarding keep working unchanged; `grouped_id` only tells clients to draw consecutive
-- members as one mosaic bubble. NULL for every existing and every non-album message.
ALTER TABLE messages ADD COLUMN grouped_id UUID;

CREATE INDEX messages_grouped_idx ON messages (room_id, grouped_id)
    WHERE grouped_id IS NOT NULL;
