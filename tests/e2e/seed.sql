-- tests/e2e/seed.sql
--
-- Seeds the E2E database with a known set of events so that query-only E2E
-- tests do not depend on the indexer running first.
--
-- Run with:
--   psql "$DATABASE_URL" -f tests/e2e/seed.sql
--
-- Or via the make target:
--   make e2e-seed

-- Contract A: 50 events across ledgers 1001–1050 (contract type)
INSERT INTO events (
    id, contract_id, event_type, tx_hash, ledger,
    ledger_closed_at, event_data, created_at
)
SELECT
    gen_random_uuid(),
    'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4',
    'contract',
    lpad(i::text, 64, '0'),
    1000 + i,
    (TIMESTAMP '2026-03-14 00:00:00' + (i || ' minutes')::interval),
    jsonb_build_object(
        'value', jsonb_build_object('i128', jsonb_build_object('hi', 0, 'lo', i * 100)),
        'topic', jsonb_build_array('transfer', 'GADDR' || i)
    ),
    NOW()
FROM generate_series(1, 50) AS i
ON CONFLICT DO NOTHING;

-- Contract B: 10 diagnostic events across ledgers 1001–1010
INSERT INTO events (
    id, contract_id, event_type, tx_hash, ledger,
    ledger_closed_at, event_data, created_at
)
SELECT
    gen_random_uuid(),
    'CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBFCT4',
    'diagnostic',
    lpad((100 + i)::text, 64, '0'),
    1000 + i,
    (TIMESTAMP '2026-03-14 00:00:00' + (i || ' minutes')::interval),
    jsonb_build_object(
        'value', jsonb_build_object('string', 'diagnostic-' || i),
        'topic', jsonb_build_array('log')
    ),
    NOW()
FROM generate_series(1, 10) AS i
ON CONFLICT DO NOTHING;

-- Contract C: 5 system events
INSERT INTO events (
    id, contract_id, event_type, tx_hash, ledger,
    ledger_closed_at, event_data, created_at
)
SELECT
    gen_random_uuid(),
    'CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCFCT4',
    'system',
    lpad((200 + i)::text, 64, '0'),
    1000 + i,
    (TIMESTAMP '2026-03-14 00:00:00' + (i || ' minutes')::interval),
    jsonb_build_object(
        'value', jsonb_build_object('string', 'system-event-' || i),
        'topic', jsonb_build_array('fee')
    ),
    NOW()
FROM generate_series(1, 5) AS i
ON CONFLICT DO NOTHING;
