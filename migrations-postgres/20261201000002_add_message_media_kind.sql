-- TG-302: classify messages that are more than text-or-attachment.
--
-- `media_kind` stays NULL for every existing row and for plain text / plain attachment
-- messages: NULL keeps today's meaning (render `content`, and `attachment` by its MIME).
-- TG-302 writes only 'sticker'; M4 owns the other kinds (docs/tg/architecture.md §4.5) and
-- any backfill. No CHECK constraint, so M4 can add kinds without rebuilding the table.
--
-- `sticker_id` records which sticker a sticker message sent. The file itself is the
-- message's own attachment row, so a NULL here (sticker hard-deleted) still renders.
ALTER TABLE messages ADD COLUMN media_kind TEXT;
ALTER TABLE messages ADD COLUMN sticker_id UUID REFERENCES stickers (id) ON DELETE SET NULL;

CREATE INDEX messages_sticker_idx ON messages (sticker_id) WHERE sticker_id IS NOT NULL;
