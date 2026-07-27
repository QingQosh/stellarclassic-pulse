-- Issue #681: Add machine learning for anomaly detection

-- Create baseline_statistics table for storing trained models
CREATE TABLE IF NOT EXISTS baseline_statistics (
    id                    UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id       UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    metric_name           TEXT        NOT NULL,
    mean                  DOUBLE PRECISION NOT NULL,
    std_dev               DOUBLE PRECISION NOT NULL,
    min                   DOUBLE PRECISION NOT NULL,
    max                   DOUBLE PRECISION NOT NULL,
    median                DOUBLE PRECISION NOT NULL,
    q1                    DOUBLE PRECISION NOT NULL,
    q3                    DOUBLE PRECISION NOT NULL,
    mad                   DOUBLE PRECISION NOT NULL,     -- Median Absolute Deviation
    sample_count          BIGINT      NOT NULL DEFAULT 0,
    training_window_days  INT         NOT NULL DEFAULT 30,
    last_updated          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(subscription_id, metric_name)
);

-- Create anomaly_alerts table
CREATE TABLE IF NOT EXISTS anomaly_alerts (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id   UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    event_id          UUID,
    metric_name       TEXT        NOT NULL,
    metric_value      DOUBLE PRECISION NOT NULL,
    expected_range    TEXT,                              -- JSON array [min, max]
    detection_method  TEXT        NOT NULL DEFAULT 'zscore', -- zscore, iqr, mad
    anomaly_score     DOUBLE PRECISION NOT NULL,
    severity          TEXT        NOT NULL DEFAULT 'medium', -- low, medium, high, critical
    alerting_enabled  BOOLEAN     NOT NULL DEFAULT true,
    acknowledged      BOOLEAN     NOT NULL DEFAULT false,
    acknowledged_at   TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create anomaly_training_jobs table for tracking model training
CREATE TABLE IF NOT EXISTS anomaly_training_jobs (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id   UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    metric_name       TEXT        NOT NULL,
    status            TEXT        NOT NULL DEFAULT 'pending', -- pending, running, completed, failed
    training_window_days INT      NOT NULL DEFAULT 30,
    samples_processed BIGINT      NOT NULL DEFAULT 0,
    error_message     TEXT,
    started_at        TIMESTAMPTZ,
    completed_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create metric_history table for tracking metrics over time
CREATE TABLE IF NOT EXISTS metric_history (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    metric_name     TEXT        NOT NULL,
    metric_value    DOUBLE PRECISION NOT NULL,
    source          TEXT,                               -- e.g., event, aggregation
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create anomaly_feedback table for model improvement
CREATE TABLE IF NOT EXISTS anomaly_feedback (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_id        UUID        NOT NULL REFERENCES anomaly_alerts(id) ON DELETE CASCADE,
    subscription_id UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    is_true_positive BOOLEAN    NOT NULL,              -- true if alert was valid
    feedback_note   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_baseline_statistics_subscription
    ON baseline_statistics(subscription_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_baseline_statistics_metric
    ON baseline_statistics(metric_name);
CREATE INDEX IF NOT EXISTS idx_anomaly_alerts_subscription
    ON anomaly_alerts(subscription_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_alerts_severity
    ON anomaly_alerts(subscription_id, severity, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_alerts_acknowledged
    ON anomaly_alerts(subscription_id, acknowledged, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_training_jobs_subscription
    ON anomaly_training_jobs(subscription_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_metric_history_subscription
    ON metric_history(subscription_id, metric_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_feedback_alert
    ON anomaly_feedback(alert_id);
