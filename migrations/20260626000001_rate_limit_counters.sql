-- Issue #567: Rate limiting per API key
-- This table stores sliding window rate limit counters for individual API keys

CREATE TABLE IF NOT EXISTS rate_limit_counters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    api_key_hash TEXT NOT NULL, -- SHA-256 hash of the API key for security
    window_start TIMESTAMPTZ NOT NULL, -- Start of the current time window
    request_count INTEGER NOT NULL DEFAULT 1, -- Number of requests in this window
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraint: API key hash should be unique per time window
    UNIQUE(api_key_hash, window_start)
);

-- Index for efficient lookup by API key hash
CREATE INDEX IF NOT EXISTS idx_rate_limit_api_key_hash ON rate_limit_counters (api_key_hash);

-- Index for cleaning up old windows
CREATE INDEX IF NOT EXISTS idx_rate_limit_window_start ON rate_limit_counters (window_start);

-- Index for finding active windows for an API key
CREATE INDEX IF NOT EXISTS idx_rate_limit_active_windows ON rate_limit_counters (api_key_hash, window_start DESC);

-- Create a function to clean up old rate limit windows (optional, for maintenance)
-- This can be called periodically to remove windows older than 24 hours
CREATE OR REPLACE FUNCTION cleanup_old_rate_limit_windows() RETURNS void AS $$
BEGIN
    DELETE FROM rate_limit_counters
    WHERE window_start < NOW() - INTERVAL '24 hours';
END;
$$ LANGUAGE plpgsql;
