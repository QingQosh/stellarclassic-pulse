-- Rollback Issue #680: Event aggregation pipelines

DROP INDEX IF EXISTS idx_aggregation_errors_rule;
DROP INDEX IF EXISTS idx_aggregation_results_window;
DROP INDEX IF EXISTS idx_aggregation_results_subscription;
DROP INDEX IF EXISTS idx_aggregation_results_rule;
DROP INDEX IF EXISTS idx_aggregation_rules_subscription;

DROP TABLE IF EXISTS aggregation_errors;
DROP TABLE IF EXISTS aggregation_results;
DROP TABLE IF EXISTS aggregation_rules;
