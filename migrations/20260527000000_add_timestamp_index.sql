-- Add an index to speed up ORDER BY timestamp queries
CREATE INDEX IF NOT EXISTS idx_events_timestamp_id ON events (timestamp DESC, id DESC);
