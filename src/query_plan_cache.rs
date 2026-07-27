use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use sqlx::PgPool;
use tracing::{debug, info};

pub const DEFAULT_PLAN_CACHE_SIZE: u64 = 1000;
pub const DEFAULT_PLAN_CACHE_TTL_SECS: u64 = 3600; // 1 hour

#[derive(Debug, Clone)]
pub struct QueryPlanCacheConfig {
    pub max_plans: u64,
    pub ttl_secs: u64,
    pub enable_prepared_statements: bool,
}

impl Default for QueryPlanCacheConfig {
    fn default() -> Self {
        Self {
            max_plans: DEFAULT_PLAN_CACHE_SIZE,
            ttl_secs: DEFAULT_PLAN_CACHE_TTL_SECS,
            enable_prepared_statements: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub query: String,
    pub plan_hash: String,
    pub estimated_cost: f64,
    pub estimated_rows: f64,
    pub actual_rows: Option<f64>,
    pub planning_time_ms: f64,
    pub execution_time_ms: Option<f64>,
}

pub struct QueryPlanCache {
    cache: Arc<Cache<String, QueryPlan>>,
    config: QueryPlanCacheConfig,
}

impl QueryPlanCache {
    pub fn new(config: QueryPlanCacheConfig) -> Self {
        let ttl = Duration::from_secs(config.ttl_secs);
        let cache = Arc::new(
            Cache::builder()
                .max_capacity(config.max_plans)
                .time_to_live(ttl)
                .build(),
        );

        info!(
            max_plans = config.max_plans,
            ttl_secs = config.ttl_secs,
            prepared_statements = config.enable_prepared_statements,
            "Initialized query plan cache"
        );

        Self { cache, config }
    }

    pub fn with_defaults() -> Self {
        Self::new(QueryPlanCacheConfig::default())
    }

    pub async fn get(&self, query: &str) -> Option<QueryPlan> {
        if let Some(plan) = self.cache.get(query).await {
            debug!(query_hash = %query_hash(query), "Query plan cache hit");
            crate::metrics::record_query_plan_cache_hit();
            return Some(plan);
        }
        debug!(query_hash = %query_hash(query), "Query plan cache miss");
        crate::metrics::record_query_plan_cache_miss();
        None
    }

    pub async fn insert(&self, query: String, plan: QueryPlan) {
        debug!(
            query_hash = %query_hash(&query),
            estimated_cost = plan.estimated_cost,
            estimated_rows = plan.estimated_rows,
            "Caching query plan"
        );
        self.cache.insert(query, plan).await;
        crate::metrics::record_query_plan_cached();
    }

    pub async fn analyze_query(&self, pool: &PgPool, query: &str) -> Result<QueryPlan, sqlx::Error> {
        // Check cache first
        if let Some(cached_plan) = self.get(query).await {
            return Ok(cached_plan);
        }

        // Analyze with EXPLAIN
        let explain_query = format!("EXPLAIN (FORMAT JSON, ANALYZE OFF) {}", query);
        let result: (String,) = sqlx::query_as(&explain_query)
            .fetch_one(pool)
            .await?;

        let plan = parse_explain_output(&result.0, query)?;
        self.insert(query.to_string(), plan.clone()).await;

        Ok(plan)
    }

    pub async fn get_cache_stats(&self) -> CacheStats {
        let count = self.cache.entry_count();
        CacheStats {
            cached_plans: count,
            max_capacity: self.config.max_plans,
        }
    }

    pub async fn clear(&self) {
        self.cache.invalidate_all();
        info!("Query plan cache cleared");
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cached_plans: u64,
    pub max_capacity: u64,
}

fn query_hash(query: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(query.as_bytes());
    format!("{:x}", hasher.finalize())[0..8].to_string()
}

fn parse_explain_output(json_str: &str, query: &str) -> Result<QueryPlan, sqlx::Error> {
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to parse EXPLAIN JSON: {}", e),
        )))?;

    let plan = value
        .get(0)
        .and_then(|p| p.get("Plan"))
        .ok_or_else(|| sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Missing Plan in EXPLAIN output",
        )))?;

    let total_cost = plan
        .get("Total Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let estimated_rows = plan
        .get("Estimated Rows")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let planning_time = value
        .get(0)
        .and_then(|p| p.get("Planning Time"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let plan_hash = query_hash(query);

    Ok(QueryPlan {
        query: query.to_string(),
        plan_hash,
        estimated_cost: total_cost,
        estimated_rows,
        actual_rows: None,
        planning_time_ms: planning_time,
        execution_time_ms: None,
    })
}

pub async fn create_pool_with_plan_cache(
    database_url: &str,
    db_max_connections: u32,
    db_min_connections: u32,
    db_statement_timeout_ms: u64,
    db_idle_timeout_secs: u64,
    db_max_lifetime_secs: u64,
    db_test_before_acquire: bool,
) -> Result<(PgPool, QueryPlanCache), sqlx::Error> {
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;

    info!(
        min_connections = db_min_connections,
        max_connections = db_max_connections,
        statement_timeout_ms = db_statement_timeout_ms,
        "Creating connection pool with plan cache"
    );

    let pool = PgPoolOptions::new()
        .max_connections(db_max_connections)
        .min_connections(db_min_connections)
        .idle_timeout(Duration::from_secs(db_idle_timeout_secs))
        .max_lifetime(Duration::from_secs(db_max_lifetime_secs))
        .test_before_acquire(db_test_before_acquire)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                conn.execute(
                    format!("SET statement_timeout = '{db_statement_timeout_ms}ms'").as_str(),
                )
                .await
                .map(|_| ())
            })
        })
        .connect(database_url)
        .await?;

    let cache = QueryPlanCache::with_defaults();

    Ok((pool, cache))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_hash_consistency() {
        let query = "SELECT * FROM events WHERE id = $1";
        let hash1 = query_hash(query);
        let hash2 = query_hash(query);
        assert_eq!(hash1, hash2, "Hash should be consistent");
    }

    #[test]
    fn query_hash_different_queries() {
        let query1 = "SELECT * FROM events WHERE id = $1";
        let query2 = "SELECT * FROM events WHERE id = $2";
        let hash1 = query_hash(query1);
        let hash2 = query_hash(query2);
        assert_ne!(hash1, hash2, "Different queries should have different hashes");
    }

    #[tokio::test]
    async fn query_plan_cache_basic() {
        let cache = QueryPlanCache::with_defaults();
        let plan = QueryPlan {
            query: "SELECT 1".to_string(),
            plan_hash: "test123".to_string(),
            estimated_cost: 100.0,
            estimated_rows: 1.0,
            actual_rows: None,
            planning_time_ms: 0.5,
            execution_time_ms: None,
        };

        cache.insert("SELECT 1".to_string(), plan.clone()).await;
        let retrieved = cache.get("SELECT 1").await;

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().estimated_cost, 100.0);
    }

    #[tokio::test]
    async fn query_plan_cache_miss() {
        let cache = QueryPlanCache::with_defaults();
        let retrieved = cache.get("SELECT nonexistent").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn query_plan_cache_stats() {
        let cache = QueryPlanCache::with_defaults();
        let plan = QueryPlan {
            query: "SELECT 1".to_string(),
            plan_hash: "test123".to_string(),
            estimated_cost: 100.0,
            estimated_rows: 1.0,
            actual_rows: None,
            planning_time_ms: 0.5,
            execution_time_ms: None,
        };

        cache.insert("SELECT 1".to_string(), plan).await;
        let stats = cache.get_cache_stats().await;

        assert_eq!(stats.cached_plans, 1);
        assert_eq!(stats.max_capacity, DEFAULT_PLAN_CACHE_SIZE);
    }

    #[tokio::test]
    async fn query_plan_cache_clear() {
        let cache = QueryPlanCache::with_defaults();
        let plan = QueryPlan {
            query: "SELECT 1".to_string(),
            plan_hash: "test123".to_string(),
            estimated_cost: 100.0,
            estimated_rows: 1.0,
            actual_rows: None,
            planning_time_ms: 0.5,
            execution_time_ms: None,
        };

        cache.insert("SELECT 1".to_string(), plan).await;
        cache.clear().await;
        let retrieved = cache.get("SELECT 1").await;

        assert!(retrieved.is_none());
    }
}
