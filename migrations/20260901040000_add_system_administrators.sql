CREATE TABLE system_admins (
    user_id TEXT PRIMARY KEY NOT NULL,
    granted_by TEXT,
    grant_source TEXT NOT NULL CHECK (grant_source IN ('bootstrap', 'administrator', 'legacy_config')),
    created_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE RESTRICT,
    FOREIGN KEY (granted_by) REFERENCES users (id) ON DELETE SET NULL
);

CREATE TABLE system_admin_events (
    id TEXT PRIMARY KEY NOT NULL,
    actor_user_id TEXT,
    subject_user_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('bootstrap', 'grant', 'revoke', 'legacy_import')),
    created_at TEXT NOT NULL,
    FOREIGN KEY (actor_user_id) REFERENCES users (id) ON DELETE SET NULL
);

CREATE INDEX system_admin_events_created_at_idx
    ON system_admin_events (created_at DESC, id DESC);

CREATE TABLE registration_invites (
    token_hash TEXT PRIMARY KEY NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    used_by TEXT,
    used_at TEXT,
    FOREIGN KEY (created_by) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (used_by) REFERENCES users (id) ON DELETE SET NULL,
    CHECK ((used_by IS NULL AND used_at IS NULL) OR (used_by IS NOT NULL AND used_at IS NOT NULL))
);

CREATE INDEX registration_invites_expiry_idx
    ON registration_invites (expires_at)
    WHERE used_at IS NULL;

INSERT INTO system_settings (key, value)
VALUES ('legacy_admin_usernames_migrated', 'false');

INSERT INTO system_settings (key, value)
VALUES ('system_admin_bootstrap_completed', 'false');
