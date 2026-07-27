use sqlx::PgConnection;
use std::time::Duration;
use tracing::{debug, error, warn};

const DEFAULT_LOCK_TIMEOUT_MS: u64 = 5000;
const DEFAULT_LOCK_WAIT_TIMEOUT_MS: u64 = 30000;

#[derive(Debug, Clone)]
pub struct AdvisoryLockConfig {
    pub acquire_timeout_ms: u64,
    pub lock_timeout_ms: u64,
    pub max_retries: u32,
}

impl Default for AdvisoryLockConfig {
    fn default() -> Self {
        Self {
            acquire_timeout_ms: DEFAULT_LOCK_WAIT_TIMEOUT_MS,
            lock_timeout_ms: DEFAULT_LOCK_TIMEOUT_MS,
            max_retries: 3,
        }
    }
}

#[derive(Debug)]
pub enum AdvisoryLockError {
    ConnectionError(String),
    LockAcquisitionTimeout,
    LockAcquisitionFailed(String),
    LockReleaseError(String),
    ValidationError(String),
}

impl std::fmt::Display for AdvisoryLockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionError(e) => write!(f, "Connection error: {}", e),
            Self::LockAcquisitionTimeout => write!(f, "Lock acquisition timed out"),
            Self::LockAcquisitionFailed(e) => write!(f, "Lock acquisition failed: {}", e),
            Self::LockReleaseError(e) => write!(f, "Lock release error: {}", e),
            Self::ValidationError(e) => write!(f, "Validation error: {}", e),
        }
    }
}

impl std::error::Error for AdvisoryLockError {}

pub struct AdvisoryLock {
    lock_id: i64,
    config: AdvisoryLockConfig,
}

impl AdvisoryLock {
    pub fn new(lock_id: i64) -> Self {
        Self {
            lock_id,
            config: AdvisoryLockConfig::default(),
        }
    }

    pub fn with_config(lock_id: i64, config: AdvisoryLockConfig) -> Self {
        Self { lock_id, config }
    }

    pub async fn validate_connection(conn: &mut PgConnection) -> Result<(), AdvisoryLockError> {
        sqlx::query("SELECT 1")
            .execute(&mut **conn)
            .await
            .map_err(|e| AdvisoryLockError::ConnectionError(e.to_string()))?;
        debug!("Database connection validated successfully");
        Ok(())
    }

    pub async fn acquire(&self, conn: &mut PgConnection) -> Result<(), AdvisoryLockError> {
        Self::validate_connection(conn).await?;

        debug!(lock_id = self.lock_id, "Attempting to acquire advisory lock");

        let timeout = Duration::from_millis(self.config.acquire_timeout_ms);
        let acquire_query = format!(
            "SELECT pg_advisory_lock_acquire($1, 0, {}) IS TRUE as acquired",
            self.config.lock_timeout_ms
        );

        for attempt in 1..=self.config.max_retries {
            match tokio::time::timeout(
                timeout,
                sqlx::query_scalar::<_, bool>(&acquire_query)
                    .bind(self.lock_id)
                    .fetch_one(&mut **conn),
            )
            .await
            {
                Ok(Ok(acquired)) if acquired => {
                    debug!(
                        lock_id = self.lock_id,
                        attempt = attempt,
                        "Advisory lock acquired successfully"
                    );
                    crate::metrics::record_advisory_lock_acquired(self.lock_id);
                    return Ok(());
                }
                Ok(Ok(_)) => {
                    warn!(
                        lock_id = self.lock_id,
                        attempt = attempt,
                        "Failed to acquire advisory lock, retrying..."
                    );
                    crate::metrics::record_advisory_lock_retry(self.lock_id);
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                }
                Ok(Err(e)) => {
                    error!(
                        lock_id = self.lock_id,
                        attempt = attempt,
                        error = %e,
                        "Database error during lock acquisition"
                    );
                    if attempt == self.config.max_retries {
                        crate::metrics::record_advisory_lock_error(self.lock_id);
                        return Err(AdvisoryLockError::LockAcquisitionFailed(e.to_string()));
                    }
                }
                Err(_) => {
                    error!(
                        lock_id = self.lock_id,
                        attempt = attempt,
                        "Lock acquisition timeout"
                    );
                    if attempt == self.config.max_retries {
                        crate::metrics::record_advisory_lock_timeout(self.lock_id);
                        return Err(AdvisoryLockError::LockAcquisitionTimeout);
                    }
                }
            }
        }

        crate::metrics::record_advisory_lock_error(self.lock_id);
        Err(AdvisoryLockError::LockAcquisitionFailed(
            "Max retries exceeded".to_string(),
        ))
    }

    pub async fn release(&self, conn: &mut PgConnection) -> Result<(), AdvisoryLockError> {
        debug!(lock_id = self.lock_id, "Attempting to release advisory lock");

        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(self.lock_id)
            .execute(&mut **conn)
            .await
            .map_err(|e| {
                error!(lock_id = self.lock_id, error = %e, "Failed to release advisory lock");
                crate::metrics::record_advisory_lock_release_error(self.lock_id);
                AdvisoryLockError::LockReleaseError(e.to_string())
            })?;

        debug!(lock_id = self.lock_id, "Advisory lock released successfully");
        crate::metrics::record_advisory_lock_released(self.lock_id);
        Ok(())
    }

    pub async fn with_lock<F, T>(
        &self,
        conn: &mut PgConnection,
        operation: F,
    ) -> Result<T, Box<dyn std::error::Error>>
    where
        F: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>,
    {
        self.acquire(conn).await?;

        let result = operation.await;

        let release_result = self.release(conn).await;

        result.and(release_result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_lock_creation() {
        let lock = AdvisoryLock::new(12345);
        assert_eq!(lock.lock_id, 12345);
    }

    #[test]
    fn advisory_lock_with_custom_config() {
        let config = AdvisoryLockConfig {
            acquire_timeout_ms: 10000,
            lock_timeout_ms: 5000,
            max_retries: 5,
        };
        let lock = AdvisoryLock::with_config(67890, config);
        assert_eq!(lock.lock_id, 67890);
        assert_eq!(lock.config.max_retries, 5);
    }

    #[test]
    fn advisory_lock_error_display() {
        let err = AdvisoryLockError::LockAcquisitionTimeout;
        assert_eq!(err.to_string(), "Lock acquisition timed out");
    }
}
