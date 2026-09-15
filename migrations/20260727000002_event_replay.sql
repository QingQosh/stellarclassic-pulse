-- Issue #679: Add event replay functionality

-- Create replay_status table to track replay operations
CREATE TABLE IF NOT EXISTS replay_status (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    start_ledger    BIGINT      NOT NULL,
    end_ledger      BIGINT      NOT NULL,
    total_events    BIGINT      NOT NULL DEFAULT 0,
    delivered_events BIGINT     NOT NULL DEFAULT 0,
    failed_events   BIGINT      NOT NULL DEFAULT 0,
    status          TEXT        NOT NULL DEFAULT 'pending', -- pending, in_progress, completed, failed
    error_message   TEXT,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create replay_delivery_log table to track individual event deliveries
CREATE TABLE IF NOT EXISTS replay_delivery_log (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    replay_id       UUID        NOT NULL REFERENCES replay_status(id) ON DELETE CASCADE,
    event_id        UUID        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'pending', -- pending, delivered, failed
    error_message   TEXT,
    attempts        INT         NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at    TIMESTAMPTZ
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_replay_status_subscription
    ON replay_status(subscription_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_replay_status_status
    ON replay_status(status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_replay_delivery_log_replay
    ON replay_delivery_log(replay_id, status);
CREATE INDEX IF NOT EXISTS idx_replay_delivery_log_event
    ON replay_delivery_log(event_id, replay_id);
