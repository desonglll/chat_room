-- TG-505: Telegram's privacy matrix.
--
-- One rule row per (account, dimension); an absent row means the Telegram default,
-- `everybody`. Exceptions name individual accounts that are always allowed or always
-- denied regardless of the tier; one account is at most one of the two per dimension
-- (the primary key), so "deny wins" is decided once, at write time, in
-- src/accounts/privacy/store.rs. `phone_number` is deliberately absent: accounts in this
-- product have no phone numbers.
CREATE TABLE user_privacy_rules (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    privacy_key TEXT NOT NULL CHECK (
        privacy_key IN ('last_seen', 'profile_photo', 'forwards', 'group_invites', 'voice_messages')
    ),
    tier TEXT NOT NULL CHECK (tier IN ('everybody', 'contacts', 'nobody')),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, privacy_key)
);

CREATE TABLE user_privacy_exceptions (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    privacy_key TEXT NOT NULL CHECK (
        privacy_key IN ('last_seen', 'profile_photo', 'forwards', 'group_invites', 'voice_messages')
    ),
    target_user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    effect TEXT NOT NULL CHECK (effect IN ('allow', 'deny')),
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, privacy_key, target_user_id),
    CHECK (user_id <> target_user_id)
);

-- Presence evaluation asks "which owners made an exception about this viewer".
CREATE INDEX user_privacy_exceptions_target_idx
ON user_privacy_exceptions (target_user_id, privacy_key);

-- Persisted last-seen. `last_seen_at` is exact and only ever shown to viewers the owner's
-- `last_seen` rule admits. `prior_seen_day` is the UTC day of the latest activity strictly
-- before the day of `last_seen_at`; the obscured tiers are computed from "latest activity
-- before today" so they can only change at UTC midnight, never at the moment of activity.
CREATE TABLE user_last_seen (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    last_seen_at TEXT NOT NULL,
    prior_seen_day TEXT
);
