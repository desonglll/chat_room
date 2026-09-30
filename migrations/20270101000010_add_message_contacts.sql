-- TG-410: a shared contact ("联系人名片"). The card is a snapshot of the shared account at send
-- time (Telegram behaviour), so it still renders if that account later changes its name or is
-- deleted (`user_id` then becomes NULL and the card offers no action).
CREATE TABLE message_contacts (
    message_id TEXT PRIMARY KEY NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
    user_id TEXT REFERENCES users (id) ON DELETE SET NULL,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    avatar_emoji TEXT NOT NULL DEFAULT ''
);
