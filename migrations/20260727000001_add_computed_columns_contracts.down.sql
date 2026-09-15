-- Rollback: Issue #690 - Add computed columns for common aggregations
-- Recreate the materialized view without the new computed columns

DROP MATERIALIZED VIEW IF EXISTS mv_contract_summary CASCADE;

DROP INDEX IF EXISTS idx_mv_contract_summary_event_count;
DROP INDEX IF EXISTS idx_mv_contract_summary_latest_event_timestamp;
DROP INDEX IF EXISTS idx_mv_contract_summary_first_event_timestamp;

CREATE MATERIALIZED VIEW mv_contract_summary AS
SELECT
    contract_id,
    COUNT(*)                                                        AS total_events,
    MIN(timestamp)                                                  AS first_event_at,
    MAX(timestamp)                                                  AS last_event_at,
    MIN(ledger)                                                     AS min_ledger,
    MAX(ledger)                                                     AS max_ledger,
    COUNT(DISTINCT tx_hash)                                         AS unique_tx_count,
    COUNT(*) FILTER (WHERE event_type = 'contract')                 AS contract_events,
    COUNT(*) FILTER (WHERE event_type = 'diagnostic')               AS diagnostic_events,
    COUNT(*) FILTER (WHERE event_type = 'system')                   AS system_events
FROM events
GROUP BY contract_id;

CREATE UNIQUE INDEX idx_mv_contract_summary_contract_id
    ON mv_contract_summary (contract_id);

CREATE INDEX IF NOT EXISTS idx_events_contract_ledger ON events(contract_id, ledger DESC);
