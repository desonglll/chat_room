-- TG-409: quoting part of a message, and replying to a message in another chat.
--
-- `reply_quote_text` / `reply_quote_offset` (UTF-16 offset into the original, like TG-304's
-- entities) hold the snippet the sender chose. A cross-chat reply also snapshots what its
-- target chat may see — the source message's sender and chat title — in `reply_source_*`,
-- because members of the target chat may have no access to the source chat: the live reply
-- join is limited to the same chat, so a cross-chat reply never exposes more of the source
-- than the sender chose to share. `reply_to_chat_id` has no foreign key: the snapshot outlives
-- the source chat. A NULL offset with a non-NULL text is a cross-chat snapshot, not a quote.
ALTER TABLE messages ADD COLUMN reply_quote_text TEXT;
ALTER TABLE messages ADD COLUMN reply_quote_offset INTEGER;
ALTER TABLE messages ADD COLUMN reply_to_chat_id TEXT;
ALTER TABLE messages ADD COLUMN reply_source_sender TEXT;
ALTER TABLE messages ADD COLUMN reply_source_chat_title TEXT;
