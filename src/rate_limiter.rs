/// Per-API Key Rate Limiting Module (Issue #567)
///
/// Implements sliding window rate limiting for individual API keys.
/// This ensures fair usage and prevents abuse of the API.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::{debug, warn};

/// Configuration for per-key rate limiting
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum requests per minute per API key
    pub requests_per_minute: Option<u32>,
    /// Maximum requests per hour per API key
    pub requests_per_hour: Option<u32>,
    /// Maximum requests per day per API key
    pub requests_per_day: Option<u32>,
}

impl RateLimitConfig {
    /// Create a new rate limit configuration
    pub fn new(
        requests_per_minute: Option<u32>,
        requests_per_hour: Option<u32>,
        requests_per_day: Option<u32>,
    ) -> Self {
        Self {
            requests_per_minute,
            requests_per_hour,
            requests_per_day,
        }
    }

    /// Create a default config (if no limits are configured)
    pub fn none() -> Self {
        Self {
            requests_per_minute: None,
            requests_per_hour: None,
            requests_per_day: None,
        }
    }

    /// Check if any rate limit is configured
    pub fn is_configured(&self) -> bool {
        self.requests_per_minute.is_some()
            || self.requests_per_hour.is_some()
            || self.requests_per_day.is_some()
    }
}

/// Rate limit status for an API key
#[derive(Clone, Debug, serde::Serialize)]
pub struct RateLimitStatus {
    /// Requests remaining in current minute
    pub remaining_minute: Option<u32>,
    /// Total requests allowed per minute
    pub limit_minute: Option<u32>,
    /// Requests remaining in current hour
    pub remaining_hour: Option<u32>,
    /// Total requests allowed per hour
    pub limit_hour: Option<u32>,
    /// Requests remaining in current day
    pub remaining_day: Option<u32>,
    /// Total requests allowed per day
    pub limit_day: Option<u32>,
    /// Whether the API key is currently rate limited
    pub is_rate_limited: bool,
    /// Unix timestamp when rate limit resets (if currently limited)
    pub reset_at: Option<i64>,
}

/// Hash an API key using SHA-256 for secure storage
fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Check and update rate limits for an API key
///
/// Returns (is_allowed, status)
pub async fn check_rate_limit(
    pool: &PgPool,
    api_key: &str,
    config: &RateLimitConfig,
) -> Result<(bool, RateLimitStatus), sqlx::Error> {
    if !config.is_configured() {
        // No rate limits configured, allow all requests
        return Ok((
            true,
            RateLimitStatus {
                remaining_minute: None,
                limit_minute: None,
                remaining_hour: None,
                limit_hour: None,
                remaining_day: None,
                limit_day: None,
                is_rate_limited: false,
                reset_at: None,
            },
        ));
    }

    let api_key_hash = hash_api_key(api_key);
    let now = Utc::now();

    // Start a transaction to ensure atomicity
    let mut tx = pool.begin().await?;

    // Check each time window
    let mut remaining_minute = config.requests_per_minute;
    let mut remaining_hour = config.requests_per_hour;
    let mut remaining_day = config.requests_per_day;
    let mut is_rate_limited = false;
    let mut reset_at: Option<i64> = None;

    // Check minute window
    if let Some(limit_per_minute) = config.requests_per_minute {
        let minute_ago = now - Duration::minutes(1);
        let (count, should_reset) =
            get_and_update_counter(&mut *tx, &api_key_hash, minute_ago, now).await?;

        remaining_minute = if count >= limit_per_minute {
            is_rate_limited = true;
            reset_at = Some((minute_ago + Duration::minutes(1)).timestamp());
            Some(0)
        } else {
            Some(limit_per_minute.saturating_sub(count))
        };
    }

    // Check hour window
    if let Some(limit_per_hour) = config.requests_per_hour {
        let hour_ago = now - Duration::hours(1);
        let (count, _) = get_and_update_counter(&mut *tx, &api_key_hash, hour_ago, now).await?;

        remaining_hour = if count >= limit_per_hour {
            is_rate_limited = true;
            if reset_at.is_none() {
                reset_at = Some((hour_ago + Duration::hours(1)).timestamp());
            }
            Some(0)
        } else {
            Some(limit_per_hour.saturating_sub(count))
        };
    }

    // Check day window
    if let Some(limit_per_day) = config.requests_per_day {
        let day_ago = now - Duration::days(1);
        let (count, _) = get_and_update_counter(&mut *tx, &api_key_hash, day_ago, now).await?;

        remaining_day = if count >= limit_per_day {
            is_rate_limited = true;
            if reset_at.is_none() {
                reset_at = Some((day_ago + Duration::days(1)).timestamp());
            }
            Some(0)
        } else {
            Some(limit_per_day.saturating_sub(count))
        };
    }

    // If not rate limited, increment the request counter
    if !is_rate_limited {
        let window_start = get_current_minute_window(now);
        sqlx::query(
            "INSERT INTO rate_limit_counters (api_key_hash, window_start, request_count, last_updated) \
             VALUES ($1, $2, 1, $3) \
             ON CONFLICT (api_key_hash, window_start) DO UPDATE \
             SET request_count = request_count + 1, last_updated = $3",
        )
        .bind(&api_key_hash)
        .bind(window_start)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let status = RateLimitStatus {
        remaining_minute,
        limit_minute: config.requests_per_minute,
        remaining_hour,
        limit_hour: config.requests_per_hour,
        remaining_day,
        limit_day: config.requests_per_day,
        is_rate_limited,
        reset_at,
    };

    Ok((is_rate_limited, status))
}

/// Get the request count and update counter for a time window
async fn get_and_update_counter(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    api_key_hash: &str,
    window_start: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(u32, bool), sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(request_count), 0) FROM rate_limit_counters \
         WHERE api_key_hash = $1 AND window_start >= $2 AND window_start <= $3",
    )
    .bind(api_key_hash)
    .bind(window_start)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;

    Ok((count as u32, false))
}

/// Get the start of the current minute window
fn get_current_minute_window(now: DateTime<Utc>) -> DateTime<Utc> {
    now.with_second(0)
        .and_then(|dt| dt.with_nanosecond(0))
        .unwrap_or(now)
}

/// Get current rate limit status for an API key (without incrementing counter)
pub async fn get_rate_limit_status(
    pool: &PgPool,
    api_key: &str,
    config: &RateLimitConfig,
) -> Result<RateLimitStatus, sqlx::Error> {
    if !config.is_configured() {
        return Ok(RateLimitStatus {
            remaining_minute: None,
            limit_minute: None,
            remaining_hour: None,
            limit_hour: None,
            remaining_day: None,
            limit_day: None,
            is_rate_limited: false,
            reset_at: None,
        });
    }

    let api_key_hash = hash_api_key(api_key);
    let now = Utc::now();

    let mut remaining_minute = config.requests_per_minute;
    let mut remaining_hour = config.requests_per_hour;
    let mut remaining_day = config.requests_per_day;
    let mut is_rate_limited = false;
    let mut reset_at: Option<i64> = None;

    // Check minute window
    if let Some(limit_per_minute) = config.requests_per_minute {
        let minute_ago = now - Duration::minutes(1);
        let count: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(request_count), 0) FROM rate_limit_counters \
             WHERE api_key_hash = $1 AND window_start >= $2",
        )
        .bind(&api_key_hash)
        .bind(minute_ago)
        .fetch_one(pool)
        .await?;

        let count = count as u32;
        remaining_minute = if count >= limit_per_minute {
            is_rate_limited = true;
            reset_at = Some((minute_ago + Duration::minutes(1)).timestamp());
            Some(0)
        } else {
            Some(limit_per_minute.saturating_sub(count))
        };
    }

    // Check hour window
    if let Some(limit_per_hour) = config.requests_per_hour {
        let hour_ago = now - Duration::hours(1);
        let count: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(request_count), 0) FROM rate_limit_counters \
             WHERE api_key_hash = $1 AND window_start >= $2",
        )
        .bind(&api_key_hash)
        .bind(hour_ago)
        .fetch_one(pool)
        .await?;

        let count = count as u32;
        remaining_hour = if count >= limit_per_hour {
            is_rate_limited = true;
            if reset_at.is_none() {
                reset_at = Some((hour_ago + Duration::hours(1)).timestamp());
            }
            Some(0)
        } else {
            Some(limit_per_hour.saturating_sub(count))
        };
    }

    // Check day window
    if let Some(limit_per_day) = config.requests_per_day {
        let day_ago = now - Duration::days(1);
        let count: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(request_count), 0) FROM rate_limit_counters \
             WHERE api_key_hash = $1 AND window_start >= $2",
        )
        .bind(&api_key_hash)
        .bind(day_ago)
        .fetch_one(pool)
        .await?;

        let count = count as u32;
        remaining_day = if count >= limit_per_day {
            is_rate_limited = true;
            if reset_at.is_none() {
                reset_at = Some((day_ago + Duration::days(1)).timestamp());
            }
            Some(0)
        } else {
            Some(limit_per_day.saturating_sub(count))
        };
    }

    Ok(RateLimitStatus {
        remaining_minute,
        limit_minute: config.requests_per_minute,
        remaining_hour,
        limit_hour: config.requests_per_hour,
        remaining_day,
        limit_day: config.requests_per_day,
        is_rate_limited,
        reset_at,
    })
}

/// Clean up old rate limit counters (should be called periodically)
pub async fn cleanup_old_counters(pool: &PgPool, hours_to_keep: i64) -> Result<u64, sqlx::Error> {
    let cutoff = Utc::now() - Duration::hours(hours_to_keep);
    let result = sqlx::query("DELETE FROM rate_limit_counters WHERE window_start < $1")
        .bind(cutoff)
        .execute(pool)
        .await?;

    debug!(
        rows_deleted = result.rows_affected(),
        "Cleaned up old rate limit counters"
    );

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = RateLimitConfig::new(Some(100), Some(1000), Some(10000));
        assert!(config.is_configured());

        let empty_config = RateLimitConfig::none();
        assert!(!empty_config.is_configured());
    }

    #[test]
    fn test_api_key_hashing() {
        let key1 = "test_key_123";
        let key2 = "test_key_123";
        let key3 = "different_key";

        assert_eq!(hash_api_key(key1), hash_api_key(key2));
        assert_ne!(hash_api_key(key1), hash_api_key(key3));
    }

    #[test]
    fn test_window_calculation() {
        let now = Utc::now();
        let window = get_current_minute_window(now);

        assert_eq!(window.second(), 0);
        assert_eq!(window.nanosecond(), 0);
        assert!(window <= now);
    }
}
