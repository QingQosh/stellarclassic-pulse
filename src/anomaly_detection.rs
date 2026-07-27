use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::VecDeque;
use uuid::Uuid;
use tracing::{info, error, warn};

/// Anomaly detection method
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DetectionMethod {
    #[serde(rename = "zscore")]
    ZScore,
    #[serde(rename = "iqr")]
    IQR,
    #[serde(rename = "mad")]
    MAD, // Median Absolute Deviation
}

/// Baseline statistics for a metric
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BaselineStatistics {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub metric_name: String,
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub mad: f64,                      // Median Absolute Deviation
    pub sample_count: i64,
    pub training_window_days: i32,
    pub last_updated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Anomaly detection alert
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnomalyAlert {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub event_id: Uuid,
    pub metric_name: String,
    pub metric_value: f64,
    pub expected_range: (f64, f64), // Will be stored as JSON
    pub detection_method: String,
    pub anomaly_score: f64,           // zscore or IQR distance
    pub severity: String,             // low, medium, high, critical
    pub alerting_enabled: bool,
    pub acknowledged: bool,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Request to create anomaly detection configuration
#[derive(Debug, Deserialize)]
pub struct CreateAnomalyDetectionRequest {
    pub metric_name: String,
    pub detection_method: DetectionMethod,
    pub z_score_threshold: Option<f64>, // Number of standard deviations (default: 3.0)
    pub iqr_multiplier: Option<f64>,    // Multiplier for IQR (default: 1.5)
    pub training_window_days: Option<i32>, // Days of history for baseline (default: 30)
    pub alerting_enabled: Option<bool>,
}

/// Response for anomaly alert
#[derive(Debug, Serialize)]
pub struct AnomalyAlertResponse {
    pub alert_id: Uuid,
    pub metric_name: String,
    pub metric_value: f64,
    pub anomaly_score: f64,
    pub severity: String,
    pub message: String,
}

/// Request to acknowledge an anomaly
#[derive(Debug, Deserialize)]
pub struct AcknowledgeAnomalyRequest {
    pub notes: Option<String>,
}

/// Calculate baseline statistics for a metric
pub async fn calculate_baseline(
    pool: &PgPool,
    subscription_id: Uuid,
    metric_name: &str,
    training_window_days: i32,
) -> Result<BaselineStatistics, String> {
    let cutoff = Utc::now() - Duration::days(training_window_days as i64);

    // Fetch historical metric values
    let values = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT metric_value FROM metric_history
         WHERE subscription_id = $1 AND metric_name = $2 AND timestamp >= $3
         ORDER BY timestamp"
    )
    .bind(subscription_id)
    .bind(metric_name)
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch metric history: {}", e))?;

    let values: Vec<f64> = values.into_iter().filter_map(|v| v).collect();

    if values.is_empty() {
        return Err(format!(
            "No data available for metric: {} in the last {} days",
            metric_name, training_window_days
        ));
    }

    // Calculate statistics
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    let std_dev = variance.sqrt();

    let mut sorted_values = values.clone();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let min = sorted_values[0];
    let max = sorted_values[sorted_values.len() - 1];
    let median = calculate_percentile(&sorted_values, 0.5);
    let q1 = calculate_percentile(&sorted_values, 0.25);
    let q3 = calculate_percentile(&sorted_values, 0.75);

    // Calculate MAD (Median Absolute Deviation)
    let deviations: Vec<f64> = sorted_values
        .iter()
        .map(|x| (x - median).abs())
        .collect();
    let mad = calculate_percentile(&deviations, 0.5);

    let baseline_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO baseline_statistics (id, subscription_id, metric_name, mean, std_dev, min, max, median, q1, q3, mad, sample_count, training_window_days, last_updated, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
    )
    .bind(baseline_id)
    .bind(subscription_id)
    .bind(metric_name)
    .bind(mean)
    .bind(std_dev)
    .bind(min)
    .bind(max)
    .bind(median)
    .bind(q1)
    .bind(q3)
    .bind(mad)
    .bind(values.len() as i64)
    .bind(training_window_days)
    .bind(Utc::now())
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to store baseline statistics: {}", e))?;

    info!(
        subscription_id = %subscription_id,
        metric_name = %metric_name,
        mean = mean,
        std_dev = std_dev,
        "Baseline statistics calculated"
    );

    Ok(BaselineStatistics {
        id: baseline_id,
        subscription_id,
        metric_name: metric_name.to_string(),
        mean,
        std_dev,
        min,
        max,
        median,
        q1,
        q3,
        mad,
        sample_count: values.len() as i64,
        training_window_days,
        last_updated: Utc::now(),
        created_at: Utc::now(),
    })
}

/// Detect anomalies using Z-score method
pub fn detect_zscore_anomaly(
    baseline: &BaselineStatistics,
    value: f64,
    threshold: f64,
) -> Option<f64> {
    if baseline.std_dev == 0.0 {
        return None;
    }

    let zscore = (value - baseline.mean).abs() / baseline.std_dev;

    if zscore > threshold {
        Some(zscore)
    } else {
        None
    }
}

/// Detect anomalies using IQR method
pub fn detect_iqr_anomaly(
    baseline: &BaselineStatistics,
    value: f64,
    multiplier: f64,
) -> Option<f64> {
    let iqr = baseline.q3 - baseline.q1;
    let lower_bound = baseline.q1 - multiplier * iqr;
    let upper_bound = baseline.q3 + multiplier * iqr;

    if value < lower_bound {
        Some((lower_bound - value).abs())
    } else if value > upper_bound {
        Some((value - upper_bound).abs())
    } else {
        None
    }
}

/// Detect anomalies using MAD (Median Absolute Deviation) method
pub fn detect_mad_anomaly(
    baseline: &BaselineStatistics,
    value: f64,
    threshold: f64,
) -> Option<f64> {
    if baseline.mad == 0.0 {
        return None;
    }

    let modified_zscore = (value - baseline.median).abs() / (1.4826 * baseline.mad);

    if modified_zscore > threshold {
        Some(modified_zscore)
    } else {
        None
    }
}

/// Record an anomaly alert
pub async fn record_anomaly_alert(
    pool: &PgPool,
    subscription_id: Uuid,
    event_id: Uuid,
    metric_name: String,
    metric_value: f64,
    expected_range: (f64, f64),
    detection_method: DetectionMethod,
    anomaly_score: f64,
    alerting_enabled: bool,
) -> Result<AnomalyAlertResponse, String> {
    let alert_id = Uuid::new_v4();

    // Determine severity based on anomaly score
    let severity = if anomaly_score > 5.0 {
        "critical"
    } else if anomaly_score > 3.0 {
        "high"
    } else if anomaly_score > 2.0 {
        "medium"
    } else {
        "low"
    };

    let method_str = match detection_method {
        DetectionMethod::ZScore => "zscore",
        DetectionMethod::IQR => "iqr",
        DetectionMethod::MAD => "mad",
    };

    sqlx::query(
        "INSERT INTO anomaly_alerts (id, subscription_id, event_id, metric_name, metric_value, expected_range, detection_method, anomaly_score, severity, alerting_enabled, acknowledged, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
    )
    .bind(alert_id)
    .bind(subscription_id)
    .bind(event_id)
    .bind(&metric_name)
    .bind(metric_value)
    .bind(format!("[{}, {}]", expected_range.0, expected_range.1))
    .bind(method_str)
    .bind(anomaly_score)
    .bind(severity)
    .bind(alerting_enabled)
    .bind(false)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to record anomaly alert: {}", e))?;

    info!(
        alert_id = %alert_id,
        subscription_id = %subscription_id,
        metric_name = %metric_name,
        anomaly_score = anomaly_score,
        severity = severity,
        "Anomaly detected and alerted"
    );

    Ok(AnomalyAlertResponse {
        alert_id,
        metric_name,
        metric_value,
        anomaly_score,
        severity: severity.to_string(),
        message: format!(
            "Anomaly detected: {} = {}, expected range: {:.2} - {:.2}",
            metric_name, metric_value, expected_range.0, expected_range.1
        ),
    })
}

/// Get anomaly alerts for a subscription
pub async fn get_anomaly_alerts(
    pool: &PgPool,
    subscription_id: Uuid,
    limit: i64,
) -> Result<Vec<AnomalyAlert>, String> {
    // Note: This is a simplified version since we stored expected_range as string
    sqlx::query(
        "SELECT id, subscription_id, event_id, metric_name, metric_value, detection_method, anomaly_score, severity, alerting_enabled, acknowledged, acknowledged_at, created_at
         FROM anomaly_alerts
         WHERE subscription_id = $1 AND acknowledged = false
         ORDER BY created_at DESC
         LIMIT $2"
    )
    .bind(subscription_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch anomaly alerts: {}", e))
    .map(|rows| {
        rows.into_iter()
            .map(|row| {
                let (id, sub_id, event_id, metric, value, method, score, severity, alert_en, ack, ack_at, created) =
                    row.into();
                AnomalyAlert {
                    id,
                    subscription_id: sub_id,
                    event_id,
                    metric_name: metric,
                    metric_value: value,
                    expected_range: (0.0, 0.0), // Simplified
                    detection_method: method,
                    anomaly_score: score,
                    severity,
                    alerting_enabled: alert_en,
                    acknowledged: ack,
                    acknowledged_at: ack_at,
                    created_at: created,
                }
            })
            .collect()
    })
}

/// Calculate percentile value
fn calculate_percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }

    let index = ((percentile * (sorted_values.len() - 1) as f64) as usize).min(sorted_values.len() - 1);
    sorted_values[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zscore_anomaly_detection() {
        let baseline = BaselineStatistics {
            id: Uuid::new_v4(),
            subscription_id: Uuid::new_v4(),
            metric_name: "test".to_string(),
            mean: 100.0,
            std_dev: 10.0,
            min: 80.0,
            max: 120.0,
            median: 100.0,
            q1: 95.0,
            q3: 105.0,
            mad: 5.0,
            sample_count: 100,
            training_window_days: 30,
            last_updated: Utc::now(),
            created_at: Utc::now(),
        };

        // Value within 3 std devs
        assert!(detect_zscore_anomaly(&baseline, 130.0, 3.0).is_none());

        // Value beyond 3 std devs
        assert!(detect_zscore_anomaly(&baseline, 140.0, 3.0).is_some());
    }

    #[test]
    fn test_iqr_anomaly_detection() {
        let baseline = BaselineStatistics {
            id: Uuid::new_v4(),
            subscription_id: Uuid::new_v4(),
            metric_name: "test".to_string(),
            mean: 100.0,
            std_dev: 10.0,
            min: 80.0,
            max: 120.0,
            median: 100.0,
            q1: 90.0,
            q3: 110.0,
            mad: 5.0,
            sample_count: 100,
            training_window_days: 30,
            last_updated: Utc::now(),
            created_at: Utc::now(),
        };

        // IQR = 20, bounds = [70, 130] with multiplier 1.5
        assert!(detect_iqr_anomaly(&baseline, 150.0, 1.5).is_some());
        assert!(detect_iqr_anomaly(&baseline, 100.0, 1.5).is_none());
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(calculate_percentile(&values, 0.5), 3.0); // median
        assert_eq!(calculate_percentile(&values, 0.25), 2.0); // Q1
        assert_eq!(calculate_percentile(&values, 0.75), 4.0); // Q3
    }
}
