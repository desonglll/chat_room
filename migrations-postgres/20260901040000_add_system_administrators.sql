CREATE TABLE system_admins (
    user_id UUID PRIMARY KEY REFERENCES users (id) ON DELETE RESTRICT,
    granted_by UUID REFERENCES users (id) ON DELETE SET NULL,
    grant_source TEXT NOT NULL CHECK (grant_source IN ('bootstrap', 'administrator', 'legacy_config')),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE system_admin_events (
    id UUID PRIMARY KEY,
    actor_user_id UUID REFERENCES users (id) ON DELETE SET NULL,
    subject_user_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('bootstrap', 'grant', 'revoke', 'legacy_import')),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX system_admin_events_created_at_idx
    ON system_admin_events (created_at DESC, id DESC);

CREATE TABLE registration_invites (
    token_hash TEXT PRIMARY KEY,
    created_by UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_by UUID REFERENCES users (id) ON DELETE SET NULL,
    used_at TIMESTAMPTZ,
    CHECK ((used_by IS NULL AND used_at IS NULL) OR (used_by IS NOT NULL AND used_at IS NOT NULL))
);

CREATE INDEX registration_invites_expiry_idx
    ON registration_invites (expires_at)
    WHERE used_at IS NULL;

INSERT INTO system_settings (key, value)
VALUES ('legacy_admin_usernames_migrated', 'false');

INSERT INTO system_settings (key, value)
VALUES ('system_admin_bootstrap_completed', 'false');
