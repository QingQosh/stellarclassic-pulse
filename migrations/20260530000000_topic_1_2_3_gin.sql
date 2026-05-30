-- GIN indexes for topic[1], topic[2], topic[3] to support server-side
-- filtering by individual topic position without a full table scan.
--
-- topic[0] is already covered by the generated column topic_0_sym and its
-- btree index (migration 20260428000000_topic_0_sym.sql).
-- These GIN expressions use jsonb containment (@>) which leverages the index.

CREATE INDEX IF NOT EXISTS idx_events_topic_1_gin
    ON events USING GIN ((event_data->'topic'->1) jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_events_topic_2_gin
    ON events USING GIN ((event_data->'topic'->2) jsonb_path_ops);

CREATE INDEX IF NOT EXISTS idx_events_topic_3_gin
    ON events USING GIN ((event_data->'topic'->3) jsonb_path_ops);
