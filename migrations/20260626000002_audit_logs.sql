-- Issue #568: Audit logging for sensitive operations
-- Comprehensive audit trail for tracking administrative actions, configuration changes, and deletions

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Event information
    event_type TEXT NOT NULL, -- 'DELETE', 'CONFIG_CHANGE', 'ADMIN_API_CALL', 'AUTH_CHANGE', etc.
    action TEXT NOT NULL,      -- Specific action (e.g., 'DELETE_SCHEMA', 'UPDATE_CONFIG')

    -- Resource information
    resource_type TEXT NOT NULL,      -- Type of resource affected (e.g., 'schema', 'config', 'api_key')
    resource_id TEXT,                  -- ID of the specific resource
    resource_description TEXT,         -- Human-readable description of resource

    -- Request information
    request_path TEXT,                 -- HTTP path that triggered the action
    request_method TEXT,               -- HTTP method (GET, POST, DELETE, PATCH, etc.)
    query_params JSONB,                -- Query parameters (if any)

    -- User/Identity information
    api_key_hash TEXT,                 -- SHA-256 hash of API key (if authenticated)
    user_email TEXT,                   -- User email (if available)
    user_id TEXT,                      -- User ID (if available)
    ip_address INET,                   -- Client IP address
    user_agent TEXT,                   -- User-Agent header

    -- Change information
    changes JSONB,                     -- JSON diff of what changed (for config changes)
    old_value JSONB,                   -- Previous value (for updates/deletes)
    new_value JSONB,                   -- New value (for updates/creates)

    -- Status information
    status_code INTEGER,               -- HTTP response status code
    success BOOLEAN NOT NULL DEFAULT true, -- Whether the action succeeded
    error_message TEXT,                -- Error message if action failed

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by TEXT,                   -- Service/component that created the log
    severity TEXT NOT NULL DEFAULT 'INFO', -- 'LOW', 'INFO', 'WARNING', 'CRITICAL'

    -- TTL/Retention
    expires_at TIMESTAMPTZ             -- When to delete this record (for retention)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_audit_logs_event_type ON audit_logs (event_type);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource_type ON audit_logs (resource_type);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource_id ON audit_logs (resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_api_key_hash ON audit_logs (api_key_hash);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_email ON audit_logs (user_email);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_ip_address ON audit_logs (ip_address);
CREATE INDEX IF NOT EXISTS idx_audit_logs_success ON audit_logs (success);
CREATE INDEX IF NOT EXISTS idx_audit_logs_severity ON audit_logs (severity);

-- Composite indexes for common queries
CREATE INDEX IF NOT EXISTS idx_audit_logs_event_created ON audit_logs (event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource_created ON audit_logs (resource_type, resource_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created ON audit_logs (user_email, created_at DESC);

-- Function to automatically set expires_at based on severity (90 days for INFO, 180 for CRITICAL)
CREATE OR REPLACE FUNCTION set_audit_log_retention() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.severity = 'CRITICAL' THEN
        NEW.expires_at := NOW() + INTERVAL '180 days';
    ELSIF NEW.severity = 'WARNING' THEN
        NEW.expires_at := NOW() + INTERVAL '90 days';
    ELSE
        NEW.expires_at := NOW() + INTERVAL '30 days';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically set retention
CREATE TRIGGER trigger_set_audit_log_retention
BEFORE INSERT ON audit_logs
FOR EACH ROW
EXECUTE FUNCTION set_audit_log_retention();

-- Function to clean up expired audit logs (run periodically)
CREATE OR REPLACE FUNCTION cleanup_expired_audit_logs() RETURNS bigint AS $$
DECLARE
    deleted_count bigint;
BEGIN
    DELETE FROM audit_logs WHERE expires_at IS NOT NULL AND expires_at < NOW();
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Create a partitioned table for high-volume scenarios (optional optimization)
-- CREATE TABLE IF NOT EXISTS audit_logs_partitioned (
--     LIKE audit_logs INCLUDING ALL
-- ) PARTITION BY RANGE (created_at);
--
-- CREATE TABLE audit_logs_2026_06 PARTITION OF audit_logs_partitioned
--     FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
