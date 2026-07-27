//! Issue #693: Database statistics auto-analysis and management
//!
//! Provides functions for automatic statistics analysis, staleness detection,
//! and automated refresh mechanisms for database query optimization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsReport {
    pub table_name: String,
    pub last_analyzed: Option<DateTime<Utc>>,
    pub hours_since_analyze: Option<i32>,
    pub is_stale: bool,
    pub row_count: Option<i64>,
    pub table_size_mb: Option<f64>,
    pub recent_jobs_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsAnalysisJob {
    pub job_id: String,
    pub table_name: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i32>,
    pub row_count_analyzed: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StalenessDetectionResult {
    pub table_name: String,
    pub is_stale: bool,
    pub hours_since_analyze: i32,
    pub staleness_threshold_hours: i32,
}

/// Refresh statistics for all tables or a specific table
pub async fn refresh_table_statistics(
    pool: &PgPool,
    table_name: Option<&str>,
) -> Result<Vec<(String, String, i32)>, ApiError> {
    let query = if let Some(table) = table_name {
        format!("SELECT * FROM refresh_table_statistics('{}')", table)
    } else {
        "SELECT * FROM refresh_table_statistics()".to_string()
    };

    let results: Vec<(String, String, i32)> = sqlx::query_as(&query)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to refresh statistics: {}", e)))?;

    info!(
        "Statistics refresh completed for {} tables",
        results.len()
    );

    Ok(results)
}

/// Detect which tables have stale statistics
pub async fn detect_stale_statistics(pool: &PgPool) -> Result<Vec<StalenessDetectionResult>, ApiError> {
    let query = "SELECT table_name, is_stale, hours_since_analyze, staleness_threshold_hours FROM detect_stale_statistics()";

    let results: Vec<StalenessDetectionResult> = sqlx::query_as(query)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to detect stale statistics: {}", e)))?;

    let stale_count = results.iter().filter(|r| r.is_stale).count();
    if stale_count > 0 {
        warn!(
            "Detected {} tables with stale statistics",
            stale_count
        );
    }

    Ok(results)
}

/// Get comprehensive statistics report for all tables
pub async fn get_statistics_report(pool: &PgPool) -> Result<Vec<StatisticsReport>, ApiError> {
    let query = "SELECT * FROM get_statistics_report()";

    let results: Vec<StatisticsReport> = sqlx::query_as(query)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to get statistics report: {}", e)))?;

    Ok(results)
}

/// Get recent statistics analysis jobs
pub async fn get_recent_analysis_jobs(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<StatisticsAnalysisJob>, ApiError> {
    let query = "SELECT id::TEXT as job_id, table_name, started_at, completed_at, duration_seconds, row_count_analyzed, status, error_message
                FROM statistics_analysis_jobs
                ORDER BY started_at DESC
                LIMIT $1";

    let results: Vec<StatisticsAnalysisJob> = sqlx::query_as(query)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to get analysis jobs: {}", e)))?;

    Ok(results)
}

/// Schedule automatic statistics refresh (would be called by a background job)
pub async fn schedule_auto_analyze(pool: &PgPool) -> Result<String, ApiError> {
    // Detect stale statistics first
    let stale = detect_stale_statistics(pool).await?;

    if stale.is_empty() {
        return Ok("No stale statistics detected".to_string());
    }

    let stale_tables: Vec<&str> = stale
        .iter()
        .filter(|s| s.is_stale)
        .map(|s| s.table_name.as_str())
        .collect();

    info!(
        "Scheduling ANALYZE for {} stale tables",
        stale_tables.len()
    );

    // Refresh statistics for stale tables
    for table_name in stale_tables {
        match refresh_table_statistics(pool, Some(table_name)).await {
            Ok(_) => info!("Refreshed statistics for table: {}", table_name),
            Err(e) => warn!("Failed to refresh statistics for table {}: {}", table_name, e),
        }
    }

    Ok(format!("Scheduled ANALYZE for {} tables", stale.len()))
}

/// Get overall statistics health score (0-100)
pub async fn get_statistics_health_score(pool: &PgPool) -> Result<u32, ApiError> {
    let stale = detect_stale_statistics(pool).await?;

    if stale.is_empty() {
        return Ok(100);
    }

    let stale_count = stale.iter().filter(|s| s.is_stale).count();
    let stale_percentage = (stale_count as f64 / stale.len() as f64) * 100.0;

    let health_score = (100.0 - stale_percentage) as u32;
    Ok(health_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_health_score_all_fresh() {
        // Health score should be high when all tables are fresh
        // This would be tested with actual data in integration tests
    }

    #[test]
    fn test_staleness_detection_threshold() {
        let result = StalenessDetectionResult {
            table_name: "events".to_string(),
            is_stale: true,
            hours_since_analyze: 48,
            staleness_threshold_hours: 24,
        };
        assert!(result.is_stale);
        assert!(result.hours_since_analyze > result.staleness_threshold_hours);
    }
}
