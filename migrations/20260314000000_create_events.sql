CREATE TABLE IF NOT EXISTS events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    ledger BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    event_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_events_contract_id ON events(contract_id);
CREATE INDEX IF NOT EXISTS idx_events_tx_hash ON events(tx_hash);
CREATE INDEX IF NOT EXISTS idx_events_ledger ON events(ledger);
CREATE UNIQUE INDEX IF NOT EXISTS idx_events_tx_hash_contract ON events(tx_hash, contract_id, event_type);
