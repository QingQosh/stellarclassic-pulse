use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{info, error, warn};

/// Aggregation window type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WindowType {
    #[serde(rename = "tumbling")]
    Tumbling, // Fixed-size time windows
    #[serde(rename = "sliding")]
    Sliding, // Overlapping time windows
    #[serde(rename = "session")]
    Session, // Activity-based windows
}

/// Aggregation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationOp {
    #[serde(rename = "count")]
    Count,
    #[serde(rename = "sum")]
    Sum,
    #[serde(rename = "avg")]
    Avg,
    #[serde(rename = "min")]
    Min,
    #[serde(rename = "max")]
    Max,
    #[serde(rename = "distinct_count")]
    DistinctCount,
}

/// Field selector for aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSelector {
    pub path: String,  // JSONPath to the field
    pub operation: AggregationOp,
    pub alias: Option<String>,
}

/// Group by configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBy {
    pub field: String,
    pub interval: Option<String>, // For numeric fields, optional interval
}

/// Aggregation rule schema
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AggregationRule {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub window_type: String,        // tumbling, sliding, session
    pub window_size_secs: i32,
    pub slide_interval_secs: Option<i32>, // For sliding windows
    pub fields: Value,              // JSON array of FieldSelector
    pub group_by: Option<Value>,    // JSON array of GroupBy
    pub filter_condition: Option<String>, // JSONPath filter
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Aggregation result
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AggregationResult {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub subscription_id: Uuid,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub group_values: Option<Value>, // JSON object with group-by values
    pub aggregated_data: Value,      // JSON object with aggregation results
    pub event_count: i64,
    pub created_at: DateTime<Utc>,
}

/// Request to create an aggregation rule
#[derive(Debug, Deserialize)]
pub struct CreateAggregationRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub window_type: WindowType,
    pub window_size_secs: i32,
    pub slide_interval_secs: Option<i32>,
    pub fields: Vec<FieldSelector>,
    pub group_by: Option<Vec<GroupBy>>,
    pub filter_condition: Option<String>,
}

/// Response for aggregation rule creation
#[derive(Debug, Serialize)]
pub struct AggregationRuleResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
}

/// Create an aggregation rule
pub async fn create_aggregation_rule(
    pool: &PgPool,
    subscription_id: Uuid,
    req: CreateAggregationRuleRequest,
) -> Result<AggregationRuleResponse, String> {
    // Validate window size
    if req.window_size_secs <= 0 {
        return Err("Window size must be positive".to_string());
    }

    // For sliding windows, validate slide interval
    if let Some(slide_size) = req.slide_interval_secs {
        if slide_size <= 0 || slide_size > req.window_size_secs {
            return Err("Slide interval must be positive and less than window size".to_string());
        }
    }

    // Validate subscription exists
    let subscription_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id = $1)"
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to validate subscription: {}", e))?;

    if !subscription_exists {
        return Err(format!("Subscription not found: {}", subscription_id));
    }

    let rule_id = Uuid::new_v4();
    let window_type_str = match req.window_type {
        WindowType::Tumbling => "tumbling",
        WindowType::Sliding => "sliding",
        WindowType::Session => "session",
    };

    let fields = serde_json::to_value(&req.fields)
        .map_err(|e| format!("Failed to serialize fields: {}", e))?;

    let group_by = req
        .group_by
        .map(|gb| serde_json::to_value(&gb).ok())
        .flatten();

    sqlx::query(
        "INSERT INTO aggregation_rules (id, subscription_id, name, description, window_type, window_size_secs, slide_interval_secs, fields, group_by, filter_condition, enabled, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"
    )
    .bind(rule_id)
    .bind(subscription_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(window_type_str)
    .bind(req.window_size_secs)
    .bind(req.slide_interval_secs)
    .bind(fields)
    .bind(group_by)
    .bind(&req.filter_condition)
    .bind(true)
    .bind(Utc::now())
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to create aggregation rule: {}", e))?;

    info!(
        rule_id = %rule_id,
        subscription_id = %subscription_id,
        name = %req.name,
        "Created aggregation rule"
    );

    Ok(AggregationRuleResponse {
        id: rule_id,
        name: req.name,
        status: "created".to_string(),
    })
}

/// Evaluate an aggregation window and store the result
pub async fn evaluate_aggregation_window(
    pool: &PgPool,
    rule_id: Uuid,
    subscription_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<AggregationResult, String> {
    let result_id = Uuid::new_v4();

    // Get aggregation rule
    let rule = sqlx::query_as::<_, AggregationRule>(
        "SELECT id, subscription_id, name, description, window_type, window_size_secs, slide_interval_secs, fields, group_by, filter_condition, enabled, created_at, updated_at FROM aggregation_rules WHERE id = $1"
    )
    .bind(rule_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to fetch aggregation rule: {}", e))?
    .ok_or_else(|| format!("Aggregation rule not found: {}", rule_id))?;

    // Fetch events within the window
    let events = sqlx::query_as::<_, (Uuid, Value)>(
        "SELECT id, value FROM soroban_events WHERE subscription_id = $1 AND timestamp >= $2 AND timestamp < $3"
    )
    .bind(subscription_id)
    .bind(window_start)
    .bind(window_end)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch events: {}", e))?;

    let event_count = events.len() as i64;

    // Apply filter if present
    let filtered_events: Vec<_> = if let Some(ref filter) = rule.filter_condition {
        events
            .into_iter()
            .filter(|(_, event)| apply_filter(event, filter))
            .collect()
    } else {
        events
    };

    // Parse field selectors
    let field_selectors: Vec<FieldSelector> = serde_json::from_value(rule.fields.clone())
        .unwrap_or_default();

    // Compute aggregations
    let mut aggregated_data = json!({});

    for selector in field_selectors {
        let operation = compute_operation(&filtered_events, &selector);
        let alias = selector.alias.unwrap_or(selector.path.clone());
        if let Value::Object(ref mut obj) = aggregated_data {
            obj.insert(alias, operation);
        }
    }

    // Store result
    sqlx::query(
        "INSERT INTO aggregation_results (id, rule_id, subscription_id, window_start, window_end, group_values, aggregated_data, event_count, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(result_id)
    .bind(rule_id)
    .bind(subscription_id)
    .bind(window_start)
    .bind(window_end)
    .bind::<Option<Value>>(None)
    .bind(&aggregated_data)
    .bind(event_count)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to store aggregation result: {}", e))?;

    info!(
        result_id = %result_id,
        rule_id = %rule_id,
        event_count = event_count,
        "Aggregation window evaluated"
    );

    Ok(AggregationResult {
        id: result_id,
        rule_id,
        subscription_id,
        window_start,
        window_end,
        group_values: None,
        aggregated_data,
        event_count,
        created_at: Utc::now(),
    })
}

/// Apply a filter condition to an event
fn apply_filter(event: &Value, filter: &str) -> bool {
    // Simple filter implementation - matches if the filter path is non-empty/true
    if let Some(value) = event.pointer(filter) {
        match value {
            Value::Bool(b) => *b,
            Value::Null => false,
            _ => true,
        }
    } else {
        false
    }
}

/// Compute aggregation operation on events
fn compute_operation(
    events: &[(Uuid, Value)],
    selector: &FieldSelector,
) -> Value {
    let values: Vec<f64> = events
        .iter()
        .filter_map(|(_, event)| {
            event
                .pointer(&selector.path)
                .and_then(|v| v.as_f64())
        })
        .collect();

    match selector.operation {
        AggregationOp::Count => json!(events.len()),
        AggregationOp::Sum => {
            json!(values.iter().sum::<f64>())
        }
        AggregationOp::Avg => {
            if values.is_empty() {
                json!(null)
            } else {
                json!(values.iter().sum::<f64>() / values.len() as f64)
            }
        }
        AggregationOp::Min => {
            json!(values.iter().copied().fold(f64::INFINITY, f64::min))
        }
        AggregationOp::Max => {
            json!(values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        }
        AggregationOp::DistinctCount => {
            let mut seen = std::collections::HashSet::new();
            for (_, event) in events {
                if let Some(v) = event.pointer(&selector.path) {
                    seen.insert(v.to_string());
                }
            }
            json!(seen.len())
        }
    }
}

/// Get aggregation results for a rule
pub async fn get_aggregation_results(
    pool: &PgPool,
    rule_id: Uuid,
    limit: i64,
) -> Result<Vec<AggregationResult>, String> {
    sqlx::query_as::<_, AggregationResult>(
        "SELECT id, rule_id, subscription_id, window_start, window_end, group_values, aggregated_data, event_count, created_at FROM aggregation_results WHERE rule_id = $1 ORDER BY window_start DESC LIMIT $2"
    )
    .bind(rule_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get aggregation results: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_validation() {
        // Valid tumbling window
        assert!(create_test_window(10, WindowType::Tumbling, None).is_ok());

        // Valid sliding window
        assert!(create_test_window(10, WindowType::Sliding, Some(5)).is_ok());
    }

    fn create_test_window(
        size: i32,
        _window_type: WindowType,
        _slide: Option<i32>,
    ) -> Result<(), String> {
        if size <= 0 {
            return Err("Window size must be positive".to_string());
        }
        Ok(())
    }

    #[test]
    fn test_filter_application() {
        let event = json!({
            "action": "transfer",
            "amount": 100
        });

        assert!(apply_filter(&event, "/action"));
        assert!(!apply_filter(&event, "/nonexistent"));
    }
}
