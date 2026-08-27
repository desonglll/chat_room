CREATE TABLE push_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    endpoint TEXT NOT NULL UNIQUE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    show_details INTEGER NOT NULL DEFAULT 0 CHECK (show_details IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX push_subscriptions_user_idx ON push_subscriptions (user_id, created_at);

CREATE TABLE push_delivery_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    notification_id TEXT NOT NULL REFERENCES notifications (id) ON DELETE CASCADE,
    subscription_id TEXT NOT NULL REFERENCES push_subscriptions (id) ON DELETE CASCADE,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NOT NULL,
    claimed_at TEXT,
    claim_token TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (notification_id, subscription_id)
);

CREATE INDEX push_delivery_jobs_ready_idx
    ON push_delivery_jobs (next_attempt_at, claimed_at, created_at);

CREATE TRIGGER push_delivery_job_insert
AFTER INSERT ON notifications
BEGIN
    INSERT INTO push_delivery_jobs (
        id, notification_id, subscription_id, next_attempt_at, created_at
    )
    SELECT NEW.id || ':' || subscription.id,
           NEW.id,
           subscription.id,
           NEW.created_at,
           NEW.created_at
    FROM push_subscriptions AS subscription
    WHERE subscription.user_id = NEW.recipient_id
    ON CONFLICT(notification_id, subscription_id) DO NOTHING;
END;

CREATE TRIGGER push_subscription_account_change
BEFORE UPDATE OF user_id ON push_subscriptions
WHEN OLD.user_id <> NEW.user_id
BEGIN
    DELETE FROM push_delivery_jobs WHERE subscription_id = OLD.id;
END;
