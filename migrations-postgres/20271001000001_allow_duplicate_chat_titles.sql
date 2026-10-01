-- TG-1210: chat titles are display names, not identifiers. Telegram lets any number of groups
-- and channels share a title; chats are addressed by id, invite link or public @username
-- (`chats_username_active_idx`, which stays unique). Lookups by title happen against the
-- in-memory chat cache, so no replacement index is needed.
DROP INDEX chats_title_active_idx;
