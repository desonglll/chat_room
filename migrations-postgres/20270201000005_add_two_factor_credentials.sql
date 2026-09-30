-- TG-506: two-step verification ("cloud password") — a second argon2 password per account.
--
-- `two_factor_credentials` exists only for accounts that turned 2FA on; absence of a row
-- is "off", so every pre-existing account keeps logging in exactly as before.
-- `recovery_email` holds only an address that was proven by a mailed code; an address
-- still waiting for its code lives in `two_factor_email_codes`.
CREATE TABLE two_factor_credentials (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    hint TEXT NOT NULL DEFAULT '',
    recovery_email TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- One outstanding mailed code per (account, purpose). `purpose` is `verify_email`
-- (prove a new recovery address) or `reset` (forgot-2FA recovery at login). Codes are
-- stored as argon2 hashes, never in clear.
CREATE TABLE two_factor_email_codes (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('verify_email', 'reset')),
    email TEXT NOT NULL,
    code_hash TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, purpose)
);

-- A login that passed the account password and now waits for the second factor. The
-- pending token itself is never stored; only its SHA-256. It is not a session.
CREATE TABLE two_factor_login_challenges (
    token_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    attempts INTEGER NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX two_factor_login_challenges_user_idx ON two_factor_login_challenges (user_id);
CREATE INDEX two_factor_login_challenges_expiry_idx ON two_factor_login_challenges (expires_at);
