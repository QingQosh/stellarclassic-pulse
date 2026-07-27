-- Issue #690: Add computed columns for common aggregations
-- This migration adds computed columns to the mv_contract_summary materialized view
-- to precompute frequently accessed aggregations

-- Add event_count, latest_event_timestamp, and first_event_timestamp columns
-- These are computed from the events table and will improve query performance

-- First, recreate the materialized view with the new computed columns
DROP MATERIALIZED VIEW IF EXISTS mv_contract_summary CASCADE;

CREATE MATERIALIZED VIEW mv_contract_summary AS
SELECT
    contract_id,
    COUNT(*)                                                        AS event_count,
    MAX(timestamp)                                                  AS latest_event_timestamp,
    MIN(timestamp)                                                  AS first_event_timestamp,
    MIN(ledger)                                                     AS min_ledger,
    MAX(ledger)                                                     AS max_ledger,
    COUNT(DISTINCT tx_hash)                                         AS unique_tx_count,
    COUNT(*) FILTER (WHERE event_type = 'contract')                 AS contract_events,
    COUNT(*) FILTER (WHERE event_type = 'diagnostic')               AS diagnostic_events,
    COUNT(*) FILTER (WHERE event_type = 'system')                   AS system_events
FROM events
GROUP BY contract_id;

-- Create unique index on contract_id for fast lookups
CREATE UNIQUE INDEX idx_mv_contract_summary_contract_id
    ON mv_contract_summary (contract_id);

-- Create indexes on computed columns for filtering queries
CREATE INDEX idx_mv_contract_summary_event_count
    ON mv_contract_summary (event_count DESC);

CREATE INDEX idx_mv_contract_summary_latest_event_timestamp
    ON mv_contract_summary (latest_event_timestamp DESC);

CREATE INDEX idx_mv_contract_summary_first_event_timestamp
    ON mv_contract_summary (first_event_timestamp ASC);

-- Ensure the composite index (contract_id, ledger DESC) exists
CREATE INDEX IF NOT EXISTS idx_events_contract_ledger ON events(contract_id, ledger DESC);
