# Audit Logging for Sensitive Operations (Issue #568)

## Overview

Soroban Pulse provides comprehensive audit logging for tracking sensitive operations including:
- **DELETE operations**: Tracks all data deletions with user and timestamp
- **Configuration changes**: Records before/after values of configuration modifications
- **Administrative API calls**: Logs all admin-level operations
- **Authentication changes**: Monitors changes to credentials and access control
- **Data exports**: Tracks when data is extracted from the system

## Features

- **Comprehensive Event Tracking**: Captures DELETE, CONFIG_CHANGE, ADMIN_API_CALL, AUTH_CHANGE, etc.
- **User Identity Tracking**: Records API key hash, user email, and IP address
- **Immutable Records**: Audit logs are designed to be tamper-evident
- **Automatic Retention**: Records automatically expire based on severity level
- **Queryable Logs**: RESTful API to query audit logs with filtering
- **Performance Optimized**: Indexed for fast queries and minimal performance impact

## Database Schema

### audit_logs Table

```sql
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,           -- DELETE, CONFIG_CHANGE, ADMIN_API_CALL, etc.
    action TEXT NOT NULL,               -- Specific action (e.g., DELETE_SCHEMA)
    resource_type TEXT NOT NULL,        -- Type of resource (schema, config, api_key, etc.)
    resource_id TEXT,                   -- ID of affected resource
    resource_description TEXT,          -- Human-readable description
    
    request_path TEXT,                  -- HTTP path (/v1/admin/...)
    request_method TEXT,                -- HTTP method (DELETE, PATCH, etc.)
    query_params JSONB,                 -- Query parameters
    
    api_key_hash TEXT,                  -- SHA-256 hash of API key
    user_email TEXT,                    -- User email if available
    user_id TEXT,                       -- User ID if available
    ip_address INET,                    -- Client IP address
    user_agent TEXT,                    -- User-Agent header
    
    changes JSONB,                      -- JSON diff of changes
    old_value JSONB,                    -- Previous value
    new_value JSONB,                    -- New value
    
    status_code INTEGER,                -- HTTP response code
    success BOOLEAN,                    -- Whether action succeeded
    error_message TEXT,                 -- Error details if failed
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by TEXT,                    -- Service that created log
    severity TEXT,                      -- LOW, INFO, WARNING, CRITICAL
    expires_at TIMESTAMPTZ              -- Retention deadline
);
```

## Event Types

### DELETE Events

When a resource is deleted:

```json
{
    "event_type": "DELETE",
    "action": "DELETE_SCHEMA",
    "resource_type": "schema",
    "resource_id": "schema_prod_123",
    "resource_description": "Production event schema",
    "old_value": { "name": "events", "fields": [...] },
    "severity": "CRITICAL"
}
```

### CONFIG_CHANGE Events

When configuration is modified:

```json
{
    "event_type": "CONFIG_CHANGE",
    "action": "UPDATE_WEBHOOK_CONFIG",
    "resource_type": "webhook_config",
    "resource_id": "webhook_1",
    "old_value": { "url": "https://old.example.com/webhook", "enabled": true },
    "new_value": { "url": "https://new.example.com/webhook", "enabled": true },
    "changes": {
        "url": { "old": "https://old.example.com/webhook", "new": "https://new.example.com/webhook" }
    },
    "severity": "WARNING"
}
```

### ADMIN_API_CALL Events

When admin endpoints are accessed:

```json
{
    "event_type": "ADMIN_API_CALL",
    "action": "PAUSE_INDEXER",
    "resource_type": "indexer",
    "request_path": "/v1/admin/indexer/pause",
    "request_method": "POST",
    "api_key_hash": "abc123...",
    "status_code": 200,
    "severity": "WARNING"
}
```

### AUTH_CHANGE Events

When authentication/authorization changes:

```json
{
    "event_type": "AUTH_CHANGE",
    "action": "API_KEY_ROTATED",
    "resource_type": "api_key",
    "resource_id": "key_123",
    "user_email": "admin@example.com",
    "severity": "CRITICAL"
}
```

## Data Retention

Audit logs are automatically retained based on severity:

| Severity | Retention Period | Purpose |
|----------|------------------|---------|
| LOW | 30 days | Development/testing events |
| INFO | 30 days | Routine operations |
| WARNING | 90 days | Important changes (configs, auth) |
| CRITICAL | 180 days | Deletions, security events |

The `expires_at` timestamp is automatically set on insertion. Expired records are cleaned up periodically.

### Custom Retention

To keep specific audit logs longer:

```sql
UPDATE audit_logs 
SET expires_at = NOW() + INTERVAL '1 year'
WHERE event_type = 'DELETE' AND resource_type = 'schema';
```

## Querying Audit Logs

### API Endpoint

**Endpoint**: `GET /v1/admin/audit-logs`

**Authentication**: Required (admin API key)

**Query Parameters**:
- `event_type`: Filter by event type (DELETE, CONFIG_CHANGE, etc.)
- `resource_type`: Filter by resource type (schema, config, etc.)
- `user_email`: Filter by user email
- `severity`: Filter by severity (LOW, INFO, WARNING, CRITICAL)
- `from_date`: Start date (ISO 8601 format)
- `to_date`: End date (ISO 8601 format)
- `limit`: Number of results (default: 100, max: 1000)
- `offset`: Pagination offset (default: 0)

### Examples

**Get all DELETE operations in the last 30 days**:
```bash
curl -H "X-Api-Key: admin_key" \
     "https://api.example.com/v1/admin/audit-logs?event_type=DELETE&limit=100"
```

**Get configuration changes by specific user**:
```bash
curl -H "X-Api-Key: admin_key" \
     "https://api.example.com/v1/admin/audit-logs?event_type=CONFIG_CHANGE&user_email=admin@example.com"
```

**Get all CRITICAL severity events**:
```bash
curl -H "X-Api-Key: admin_key" \
     "https://api.example.com/v1/admin/audit-logs?severity=CRITICAL"
```

**Get failed operations**:
```bash
curl -H "X-Api-Key: admin_key" \
     "https://api.example.com/v1/admin/audit-logs?success=false"
```

## Programmatic Usage

### Log a Deletion

```rust
use soroban_pulse::audit_logging::{
    AuditLogEntry, AuditEventType, AuditSeverity, log_audit
};

let entry = AuditLogEntry::new(
    AuditEventType::Delete,
    "DELETE_SCHEMA",
    "schema"
)
.with_resource_id("schema_123")
.with_resource_description("Production event schema")
.with_api_key("user_api_key")
.with_user_email("admin@example.com")
.with_request("/v1/admin/schemas/schema_123", "DELETE")
.with_status_code(200)
.with_severity(AuditSeverity::Critical);

let id = log_audit(&pool, &entry).await?;
```

### Log a Configuration Change

```rust
let entry = AuditLogEntry::new(
    AuditEventType::ConfigChange,
    "UPDATE_WEBHOOK_URL",
    "webhook_config"
)
.with_resource_id("webhook_1")
.with_old_value(Some(json!({
    "url": "https://old.example.com/webhook",
    "enabled": true
})))
.with_new_value(Some(json!({
    "url": "https://new.example.com/webhook",
    "enabled": true
})))
.with_changes(json!({
    "url": {
        "old": "https://old.example.com/webhook",
        "new": "https://new.example.com/webhook"
    }
}))
.with_api_key("admin_key")
.with_severity(AuditSeverity::Warning);

log_audit(&pool, &entry).await?;
```

### Query Audit Logs

```rust
use soroban_pulse::audit_logging::{AuditLogQuery, query_audit_logs};

let query = AuditLogQuery::new()
    .with_event_type("DELETE")
    .with_severity("CRITICAL")
    .with_pagination(50, 0);

let results = query_audit_logs(&pool, &query).await?;

for log in results {
    println!("Event: {}", log["event_type"]);
    println!("Resource: {}", log["resource_type"]);
    println!("User: {}", log["user_email"]);
    println!("Time: {}", log["created_at"]);
}
```

## Security Considerations

### 1. Immutability

Audit logs should be immutable. Consider:
- Making `audit_logs` table read-only for non-admin users
- Using database-level triggers to prevent direct updates
- Archiving logs to write-once storage periodically

```sql
-- Prevent updates to audit logs (only allow inserts/deletes on expiry)
CREATE POLICY audit_logs_immutable ON audit_logs
    FOR UPDATE USING (false);
```

### 2. API Key Hashing

API keys are hashed with SHA-256 before storage:
- Never stores plaintext keys
- Allows correlation without exposing keys
- Prevents accidental exposure in logs

### 3. IP Address Tracking

Records client IP for forensic analysis:
- Identifies source of requests
- Detects suspicious patterns
- Useful for investigation

### 4. Sensitive Data Filtering

Be careful when logging:
- Avoid logging full request/response bodies
- Hash or redact sensitive fields
- Consider compliance requirements (GDPR, HIPAA, etc.)

## Best Practices

### 1. Regular Audits

```bash
# Weekly audit report of all DELETE operations
curl -s -H "X-Api-Key: admin_key" \
     "https://api.example.com/v1/admin/audit-logs?event_type=DELETE" \
     | jq '.' > /tmp/audit_report_$(date +%Y-%m-%d).json
```

### 2. Monitor Critical Operations

Set up alerts for:
- All DELETE operations
- Configuration changes
- Failed admin API calls
- Unusual user activity

```sql
-- Find all CRITICAL events in the last 24 hours
SELECT * FROM audit_logs 
WHERE severity = 'CRITICAL' 
  AND created_at > NOW() - INTERVAL '24 hours'
ORDER BY created_at DESC;
```

### 3. Archive Important Logs

```sql
-- Archive production deletion records to separate table
CREATE TABLE audit_logs_archive AS
SELECT * FROM audit_logs 
WHERE event_type = 'DELETE' 
  AND created_at < NOW() - INTERVAL '90 days';

DELETE FROM audit_logs
WHERE id IN (SELECT id FROM audit_logs_archive);
```

### 4. Compliance Reporting

Generate compliance reports:

```sql
-- Users who performed deletions
SELECT DISTINCT user_email, COUNT(*) as deletion_count
FROM audit_logs
WHERE event_type = 'DELETE'
  AND created_at > NOW() - INTERVAL '30 days'
GROUP BY user_email
ORDER BY deletion_count DESC;
```

## Troubleshooting

### Logs Not Being Created

**Check**:
1. Is `audit_logs` table present? Run `\dt audit_logs` in psql
2. Are migrations running? Check migration status
3. Is the code using the audit logging module?

**Fix**:
```bash
# Run migrations manually
sqlx migrate run --database-url $DATABASE_URL
```

### Query Performance Issues

**Problem**: Slow audit log queries

**Solution**:
1. Add appropriate indexes (already created)
2. Limit query date ranges
3. Archive old logs to separate table
4. Use pagination (limit + offset)

### Retention Not Working

**Check**:
1. Are triggers enabled?
2. Is cleanup function running?
3. Check `expires_at` values

**Fix**:
```sql
-- Check trigger status
SELECT * FROM information_schema.triggers 
WHERE trigger_name = 'trigger_set_audit_log_retention';

-- Manually clean up
SELECT cleanup_expired_audit_logs();
```

## See Also

- [API Security](./api_security.md)
- [Configuration Management](./configuration.md)
- [Admin Operations](./admin_operations.md)

## References

- [OWASP Audit Logging Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)
- [CIS Benchmarks - Logging](https://www.cisecurity.org/cis-benchmarks/)
- [ISO 27001 - Audit Trail](https://www.iso.org/isoiec-27001-information-security-management.html)
