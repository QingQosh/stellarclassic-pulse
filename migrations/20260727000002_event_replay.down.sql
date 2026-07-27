-- Rollback Issue #679: Event replay functionality

DROP INDEX IF EXISTS idx_replay_delivery_log_event;
DROP INDEX IF EXISTS idx_replay_delivery_log_replay;
DROP INDEX IF EXISTS idx_replay_status_status;
DROP INDEX IF EXISTS idx_replay_status_subscription;

DROP TABLE IF EXISTS replay_delivery_log;
DROP TABLE IF EXISTS replay_status;
