CREATE TABLE push_subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    endpoint TEXT NOT NULL UNIQUE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    show_details BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX push_subscriptions_user_idx ON push_subscriptions (user_id, created_at);

CREATE TABLE push_delivery_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    notification_id TEXT NOT NULL REFERENCES notifications (id) ON DELETE CASCADE,
    subscription_id TEXT NOT NULL REFERENCES push_subscriptions (id) ON DELETE CASCADE,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL,
    claimed_at TIMESTAMPTZ,
    claim_token TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (notification_id, subscription_id)
);

CREATE INDEX push_delivery_jobs_ready_idx
    ON push_delivery_jobs (next_attempt_at, claimed_at, created_at);

CREATE FUNCTION create_push_delivery_jobs() RETURNS TRIGGER AS $$
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
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER push_delivery_job_insert
AFTER INSERT ON notifications
FOR EACH ROW EXECUTE FUNCTION create_push_delivery_jobs();

CREATE FUNCTION clear_push_jobs_on_account_change() RETURNS TRIGGER AS $$
BEGIN
    IF OLD.user_id <> NEW.user_id THEN
        DELETE FROM push_delivery_jobs WHERE subscription_id = OLD.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER push_subscription_account_change
BEFORE UPDATE OF user_id ON push_subscriptions
FOR EACH ROW EXECUTE FUNCTION clear_push_jobs_on_account_change();
