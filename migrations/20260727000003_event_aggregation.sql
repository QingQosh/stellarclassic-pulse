-- Issue #680: Implement event aggregation pipelines

-- Create aggregation_rules table
CREATE TABLE IF NOT EXISTS aggregation_rules (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subscription_id     UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    name                TEXT        NOT NULL,
    description         TEXT,
    window_type         TEXT        NOT NULL DEFAULT 'tumbling', -- tumbling, sliding, session
    window_size_secs    INT         NOT NULL,
    slide_interval_secs INT,
    fields              JSONB       NOT NULL,            -- Array of FieldSelector
    group_by            JSONB,                           -- Array of GroupBy
    filter_condition    TEXT,                            -- JSONPath filter
    enabled             BOOLEAN     NOT NULL DEFAULT true,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create aggregation_results table
CREATE TABLE IF NOT EXISTS aggregation_results (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id         UUID        NOT NULL REFERENCES aggregation_rules(id) ON DELETE CASCADE,
    subscription_id UUID        NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    window_start    TIMESTAMPTZ NOT NULL,
    window_end      TIMESTAMPTZ NOT NULL,
    group_values    JSONB,                              -- JSON object with group-by values
    aggregated_data JSONB       NOT NULL,               -- JSON object with aggregation results
    event_count     BIGINT      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create aggregation_errors table for tracking failed aggregations
CREATE TABLE IF NOT EXISTS aggregation_errors (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id         UUID        NOT NULL REFERENCES aggregation_rules(id) ON DELETE CASCADE,
    window_start    TIMESTAMPTZ NOT NULL,
    window_end      TIMESTAMPTZ NOT NULL,
    error_message   TEXT        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_aggregation_rules_subscription
    ON aggregation_rules(subscription_id, enabled, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_aggregation_results_rule
    ON aggregation_results(rule_id, window_start DESC);
CREATE INDEX IF NOT EXISTS idx_aggregation_results_subscription
    ON aggregation_results(subscription_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_aggregation_results_window
    ON aggregation_results(rule_id, window_start, window_end);
CREATE INDEX IF NOT EXISTS idx_aggregation_errors_rule
    ON aggregation_errors(rule_id, created_at DESC);
