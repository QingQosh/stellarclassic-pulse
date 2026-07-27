CREATE TABLE IF NOT EXISTS rate_limit_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_url TEXT NOT NULL UNIQUE,
    per_minute_limit INT NOT NULL DEFAULT 60 CHECK (per_minute_limit > 0),
    window_start TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    window_count INT NOT NULL DEFAULT 0 CHECK (window_count >= 0),
    consecutive_failures INT NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
    backoff_until TIMESTAMPTZ,
    health_status TEXT NOT NULL DEFAULT 'healthy'
        CHECK (health_status IN ('healthy', 'degraded', 'unhealthy')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_rate_limit_endpoints_backoff
    ON rate_limit_endpoints (backoff_until)
    WHERE backoff_until IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_rate_limit_endpoints_health
    ON rate_limit_endpoints (health_status);
