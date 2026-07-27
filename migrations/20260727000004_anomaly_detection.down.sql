-- Rollback Issue #681: Machine learning for anomaly detection

DROP INDEX IF EXISTS idx_anomaly_feedback_alert;
DROP INDEX IF EXISTS idx_metric_history_subscription;
DROP INDEX IF EXISTS idx_anomaly_training_jobs_subscription;
DROP INDEX IF EXISTS idx_anomaly_alerts_acknowledged;
DROP INDEX IF EXISTS idx_anomaly_alerts_severity;
DROP INDEX IF EXISTS idx_anomaly_alerts_subscription;
DROP INDEX IF EXISTS idx_baseline_statistics_metric;
DROP INDEX IF EXISTS idx_baseline_statistics_subscription;

DROP TABLE IF EXISTS anomaly_feedback;
DROP TABLE IF EXISTS metric_history;
DROP TABLE IF EXISTS anomaly_training_jobs;
DROP TABLE IF EXISTS anomaly_alerts;
DROP TABLE IF EXISTS baseline_statistics;
