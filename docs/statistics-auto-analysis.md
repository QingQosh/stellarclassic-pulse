# Database Statistics Auto-Analysis Configuration

## Overview

The Statistics Auto-Analysis feature automatically maintains optimal database query performance by monitoring and refreshing table statistics. PostgreSQL uses these statistics to generate optimal query plans, and keeping them current is critical for performance.

## Features

### 1. Automatic Statistics Detection
- **Staleness Detection**: Identifies tables with outdated statistics
- **Configurable Thresholds**: Customize staleness detection per table
- **Automatic Triggers**: Tracks changes to mark statistics stale when data is modified

### 2. Statistics Refresh
- **Scheduled ANALYZE Jobs**: Automatically run `ANALYZE` on stale tables
- **Job Tracking**: Monitor analysis job history and performance
- **Error Handling**: Detailed error logging for failed analysis jobs

### 3. Health Monitoring
- **Health Score**: Overall statistics health on 0-100 scale
- **Per-Table Metrics**: Size, row count, and analysis recency
- **Job Statistics**: Track analysis performance and duration

## Database Objects

### Tables

#### `table_statistics_metadata`
Tracks statistics metadata for all tables:
```sql
CREATE TABLE table_statistics_metadata (
    table_name TEXT PRIMARY KEY,
    last_analyzed TIMESTAMPTZ,
    last_vacuumed TIMESTAMPTZ,
    row_count BIGINT,
    live_row_count BIGINT,
    dead_row_count BIGINT,
    table_size_bytes BIGINT,
    is_stale BOOLEAN,
    staleness_threshold_hours INT DEFAULT 24,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);
```

#### `statistics_analysis_jobs`
History of ANALYZE jobs executed:
```sql
CREATE TABLE statistics_analysis_jobs (
    id UUID PRIMARY KEY,
    table_name TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    duration_seconds INT,
    row_count_analyzed BIGINT,
    status TEXT,  -- 'running', 'success', 'error'
    error_message TEXT,
    created_at TIMESTAMPTZ
);
```

### Functions

#### `refresh_table_statistics(target_table TEXT)`
Executes ANALYZE on specified tables and updates metadata:
```sql
SELECT * FROM refresh_table_statistics();  -- All tables
SELECT * FROM refresh_table_statistics('events');  -- Specific table
```

Returns:
- `table_name` - Table analyzed
- `status` - 'success' or error message
- `duration_seconds` - Time taken to analyze

#### `detect_stale_statistics()`
Identifies tables with outdated statistics:
```sql
SELECT * FROM detect_stale_statistics();
```

Returns:
- `table_name` - Table name
- `is_stale` - Boolean stale flag
- `hours_since_analyze` - Hours since last ANALYZE
- `staleness_threshold_hours` - Configured threshold

#### `get_statistics_report()`
Comprehensive statistics report for all tables:
```sql
SELECT * FROM get_statistics_report();
```

Returns detailed metrics including size, row count, and recent job count.

#### `initialize_statistics_tracking()`
Sets up tracking for all existing tables:
```sql
SELECT initialize_statistics_tracking();
```

### Triggers

#### `mark_events_stats_stale`
Automatically marks statistics stale when events table is modified:
- Fires on INSERT, UPDATE, DELETE
- Updates `is_stale = TRUE` in metadata table

## REST API Endpoints

All endpoints require admin authentication (ADMIN_API_KEY header).

### GET /v1/admin/statistics/report
Get comprehensive statistics report for all tables:
```bash
curl -H "X-API-Key: $ADMIN_API_KEY" \
  https://api.soroban-pulse.com/v1/admin/statistics/report
```

Response:
```json
[
  {
    "table_name": "events",
    "last_analyzed": "2026-07-27T12:00:00Z",
    "hours_since_analyze": 2,
    "is_stale": false,
    "row_count": 1000000,
    "table_size_mb": 250.5,
    "recent_jobs_count": 1
  }
]
```

### GET /v1/admin/statistics/stale
Detect tables with stale statistics:
```bash
curl -H "X-API-Key: $ADMIN_API_KEY" \
  https://api.soroban-pulse.com/v1/admin/statistics/stale
```

Response:
```json
[
  {
    "table_name": "events",
    "is_stale": true,
    "hours_since_analyze": 48,
    "staleness_threshold_hours": 24
  }
]
```

### GET /v1/admin/statistics/health
Get overall statistics health score:
```bash
curl -H "X-API-Key: $ADMIN_API_KEY" \
  https://api.soroban-pulse.com/v1/admin/statistics/health
```

Response:
```json
{
  "health_score": 85,
  "status": "healthy",
  "stale_tables_count": 1,
  "total_tables": 10,
  "timestamp": "2026-07-27T12:00:00Z"
}
```

Health Levels:
- **100-80**: Healthy - All statistics are current
- **79-60**: Degraded - Some tables have stale statistics
- **<60**: Critical - Multiple tables need immediate analysis

### POST /v1/admin/statistics/refresh
Refresh statistics for all tables or a specific table:
```bash
# Refresh all tables
curl -X POST -H "X-API-Key: $ADMIN_API_KEY" \
  https://api.soroban-pulse.com/v1/admin/statistics/refresh

# Refresh specific table
curl -X POST -H "X-API-Key: $ADMIN_API_KEY" \
  "https://api.soroban-pulse.com/v1/admin/statistics/refresh?table_name=events"
```

Response:
```json
{
  "message": "Refreshed statistics for 10 tables",
  "results_count": 10,
  "timestamp": "2026-07-27T12:00:00Z"
}
```

### GET /v1/admin/statistics/jobs
Get recent statistics analysis jobs:
```bash
# Get last 50 jobs (default)
curl -H "X-API-Key: $ADMIN_API_KEY" \
  https://api.soroban-pulse.com/v1/admin/statistics/jobs

# Get last 100 jobs
curl -H "X-API-Key: $ADMIN_API_KEY" \
  "https://api.soroban-pulse.com/v1/admin/statistics/jobs?limit=100"
```

Response:
```json
[
  {
    "job_id": "550e8400-e29b-41d4-a716-446655440000",
    "table_name": "events",
    "started_at": "2026-07-27T12:00:00Z",
    "completed_at": "2026-07-27T12:00:15Z",
    "duration_seconds": 15,
    "row_count_analyzed": 1000000,
    "status": "success",
    "error_message": null
  }
]
```

## Configuration

### Staleness Threshold
The default staleness threshold is 24 hours. Customize per table:
```sql
UPDATE table_statistics_metadata
SET staleness_threshold_hours = 12
WHERE table_name = 'events';
```

### Automatic Scheduling

To enable automatic statistics refresh via pg_cron:

```sql
-- Install pg_cron extension if not installed
CREATE EXTENSION IF NOT EXISTS pg_cron;

-- Schedule automatic stats refresh daily at 2 AM
SELECT cron.schedule(
  'refresh-stats-daily',
  '0 2 * * *',  -- 2 AM every day
  'SELECT refresh_table_statistics()'
);

-- Schedule stale detection check every 6 hours
SELECT cron.schedule(
  'detect-stale-stats',
  '0 */6 * * *',  -- Every 6 hours
  'SELECT detect_stale_statistics()'
);

-- Schedule cleanup of old jobs monthly
SELECT cron.schedule(
  'cleanup-old-jobs',
  '0 3 1 * *',  -- 3 AM on 1st of month
  'DELETE FROM statistics_analysis_jobs WHERE started_at < NOW() - INTERVAL ''30 days'''
);
```

View scheduled jobs:
```sql
SELECT * FROM cron.job;
```

## Monitoring

### Query Estimated Rows vs Actual Rows
To check if statistics are accurate:
```sql
SELECT
  schemaname,
  tablename,
  (SELECT n_live_tup FROM pg_stat_user_tables 
   WHERE relname = t.tablename) AS live_rows,
  (SELECT n_dead_tup FROM pg_stat_user_tables 
   WHERE relname = t.tablename) AS dead_rows,
  (SELECT last_analyze FROM pg_stat_user_tables 
   WHERE relname = t.tablename) AS last_analyzed
FROM pg_tables t
WHERE schemaname = 'public'
ORDER BY schemaname, tablename;
```

### Check Query Plans
Review actual query plans with statistics:
```sql
EXPLAIN ANALYZE SELECT COUNT(*) FROM events WHERE timestamp > NOW() - INTERVAL '7 days';
```

## Performance Impact

- **Analysis Duration**: Typically 1-15 seconds depending on table size
- **Lock Level**: `SHARE UPDATE EXCLUSIVE` (brief, low impact)
- **Recommended Frequency**: 
  - High-churn tables: 12-24 hours
  - Medium-churn tables: 24-48 hours
  - Low-churn tables: Weekly

## Troubleshooting

### Statistics show as stale but were just refreshed
Check the staleness threshold:
```sql
SELECT table_name, last_analyzed, staleness_threshold_hours
FROM table_statistics_metadata
WHERE is_stale = TRUE;
```

### Analysis jobs failing
Check job history for errors:
```sql
SELECT job_id, table_name, status, error_message, duration_seconds
FROM statistics_analysis_jobs
WHERE status = 'error'
ORDER BY started_at DESC
LIMIT 10;
```

### Performance degradation after statistics refresh
This is usually temporary. Query plans are cached in PostgreSQL. Run:
```sql
-- Clear query cache if using pg_stat_statements
SELECT pg_stat_statements_reset();
```

## Related Issues
- Issue #690: Add computed columns for common aggregations
- Issue #691: Implement table partitioning by time
- Issue #692: Add connection pooling configuration UI
