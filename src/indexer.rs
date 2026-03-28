use chrono::DateTime;
use reqwest::Client;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn, instrument, span, Level};

use crate::{
    config::{Config, HealthState, IndexerState},
    metrics,
    models::{GetEventsResult, LatestLedgerResult, RpcResponse, SorobanEvent},
};

#[derive(Debug, thiserror::Error)]
enum IndexerFetchError {
    #[error("{0}")]
    Rpc(String),
    #[error(transparent)]
    DbConnection(#[from] sqlx::Error),
}

fn build_rpc_client(config: &Config) -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(config.rpc_connect_timeout_secs))
        .timeout(Duration::from_secs(config.rpc_request_timeout_secs))
        .pool_max_idle_per_host(5)
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .expect("Failed to build HTTP client")
}

pub struct Indexer {
    pool: PgPool,
    client: Client,
    config: Config,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    health_state: Option<Arc<HealthState>>,
    indexer_state: Option<Arc<IndexerState>>,
}

impl Indexer {
    pub fn new(pool: PgPool, config: Config, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Self {
        let client = build_rpc_client(&config);

        Self {
            pool,
            client,
            config,
            shutdown_rx,
            health_state: None,
            indexer_state: None,
        }
    }

    /// Set the health state for updating the last poll timestamp
    pub fn set_health_state(&mut self, health_state: Arc<HealthState>) {
        self.health_state = Some(health_state);
    }

    /// Set the indexer state for exposing operational metrics to the /status endpoint
    pub fn set_indexer_state(&mut self, indexer_state: Arc<IndexerState>) {
        self.indexer_state = Some(indexer_state);
    }

    pub async fn run(&self) {
        let mut current_ledger = self.config.start_ledger;
        let mut consecutive_db_errors = 0u32;

        if current_ledger == 0 {
            let mut retries = 0;
            loop {
                match self.get_latest_ledger().await {
                    Ok(ledger) => {
                        current_ledger = ledger;
                        info!(ledger = current_ledger, "Starting from latest ledger");
                        metrics::update_current_ledger(current_ledger);
                        if let Some(ref s) = self.indexer_state {
                            s.current_ledger.store(current_ledger, std::sync::atomic::Ordering::Relaxed);
                        }
                        break;
                    }
                    Err(e) => {
                        error!(attempt = retries + 1, error = %e, "Failed to get latest ledger");
                        retries += 1;
                        if retries >= 5 {
                            if self.config.start_ledger_fallback {
                                warn!("Falling back to genesis ledger (1) due to RPC failure");
                                current_ledger = 1;
                                break;
                            } else {
                                error!("Fatal RPC error: Could not fetch initial ledger after 5 attempts");
                                std::process::exit(1);
                            }
                        }
                        sleep(Duration::from_secs(10)).await;
                    }
                }
            }
        }

        loop {
            if *self.shutdown_rx.borrow() {
                info!("Indexer shutting down gracefully");
                break;
            }

            match self.fetch_and_store_events(current_ledger).await {
                Ok(latest) => {
                    consecutive_db_errors = 0;
                    // Update the last poll timestamp on success
                    if let Some(ref health_state) = self.health_state {
                        health_state.update_last_poll();
                    }
                    if latest > current_ledger {
                        current_ledger = latest;
                        metrics::update_current_ledger(current_ledger);
                        if let Some(ref s) = self.indexer_state {
                            s.current_ledger.store(current_ledger, std::sync::atomic::Ordering::Relaxed);
                        }

                        // Calculate and update lag
                        let latest_ledger = self.get_latest_ledger().await.unwrap_or(0);
                        if latest_ledger > current_ledger {
                            if let Some(ref s) = self.indexer_state {
                                s.latest_ledger.store(latest_ledger, std::sync::atomic::Ordering::Relaxed);
                            }
                            let lag = latest_ledger - current_ledger;
                            metrics::update_indexer_lag(lag);
                            
                            // Warn if lag exceeds threshold
                            if lag > self.config.indexer_lag_warn_threshold {
                                warn!(
                                    lag = lag,
                                    threshold = self.config.indexer_lag_warn_threshold,
                                    "Indexer is falling behind"
                                );
                            }
                        }
                    } else {
                        sleep(Duration::from_secs(5)).await;
                    }
                }
                Err(IndexerFetchError::DbConnection(e)) => {
                    consecutive_db_errors += 1;
                    let backoff_secs = if consecutive_db_errors >= 5 {
                        60
                    } else {
                        10
                    };
                    if consecutive_db_errors == 5 {
                        error!(
                            consecutive = consecutive_db_errors,
                            "DB unavailable, backing off"
                        );
                    } else if consecutive_db_errors < 5 {
                        error!(error = %e, "Indexer error");
                    }
                    sleep(Duration::from_secs(backoff_secs)).await;
                }
                Err(IndexerFetchError::Rpc(msg)) => {
                    error!(error = %msg, "Indexer error");
                    sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }

    async fn get_latest_ledger(&self) -> Result<u64, String> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestLedger"
        });

        let span = span!(Level::INFO, "rpc_get_latest_ledger", url = %self.config.stellar_rpc_url);
        let _enter = span.enter();

        let resp: RpcResponse<LatestLedgerResult> = self
            .client
            .post(&self.config.stellar_rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    warn!(
                        timeout_secs = self.config.rpc_request_timeout_secs,
                        "RPC request timeout"
                    );
                }
                metrics::record_rpc_error();
                e.to_string()
            })?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        match resp.result {
            Some(r) => {
                metrics::update_latest_ledger(r.sequence);
                Ok(r.sequence)
            }
            None => {
                if let Some(err) = resp.error {
                    warn!(code = err.code, message = %err.message, "RPC error");
                    metrics::record_rpc_error();
                }
                Err("RPC returned no result".to_string())
            }
        }
    }

    #[instrument(skip(self), fields(start_ledger = start_ledger))]
    async fn fetch_and_store_events(&self, start_ledger: u64) -> Result<u64, IndexerFetchError> {
        let mut cursor: Option<String> = None;
        let mut latest_ledger = start_ledger;
        let mut total_fetched = 0;
        let mut total_inserted = 0;
        let mut total_skipped = 0;

        loop {
            let mut params = json!({
                "filters": [],
                "pagination": { "limit": 100 }
            });

            if let Some(c) = &cursor {
                params["pagination"]["cursor"] = json!(c);
            } else {
                params["startLedger"] = json!(start_ledger);
            }

            let body = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getEvents",
                "params": params
            });

            let resp: RpcResponse<GetEventsResult> = self
                .client
                .post(&self.config.stellar_rpc_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        warn!(
                            timeout_secs = self.config.rpc_request_timeout_secs,
                            "RPC request timeout"
                        );
                    }
                    metrics::record_rpc_error();
                    IndexerFetchError::Rpc(e.to_string())
                })?
                .json()
                .await
                .map_err(|e| IndexerFetchError::Rpc(e.to_string()))?;

            let result = match resp.result {
                Some(r) => r,
                None => {
                    if let Some(err) = resp.error {
                        warn!(code = err.code, message = %err.message, "RPC error");
                        metrics::record_rpc_error();
                        return Err(IndexerFetchError::Rpc(err.message));
                    }
                    break;
                }
            };

            latest_ledger = result.latest_ledger;
            let current_count = result.events.len();
            total_fetched += current_count;

            for event in result.events {
                match self.store_event(&event).await {
                    Ok(rows) => {
                        total_inserted += rows;
                        if rows == 0 {
                            total_skipped += 1;
                        }
                    }
                    Err(e) => {
                        warn!(
                            tx_hash = %event.tx_hash,
                            contract_id = %event.contract_id,
                            ledger = event.ledger,
                            event_type = %event.event_type,
                            error = %e,
                            "Failed to store event",
                        );
                    }
                }
            }

            cursor = result.rpc_cursor;
            if cursor.is_none() {
                break;
            }
        }

        info!(
            fetched = total_fetched,
            inserted = total_inserted,
            ledger = latest_ledger,
            "Indexed ledger range"
        );
        metrics::record_events_indexed(total_inserted as u64);

        let _duplicate_events_skipped = total_skipped;

        if latest_ledger > start_ledger {
            Ok(latest_ledger + 1)
        } else {
            Ok(start_ledger)
        }
    }
    #[instrument(skip(self, event), fields(tx_hash = %event.tx_hash, contract_id = %event.contract_id, ledger = event.ledger))]
    async fn store_event(&self, event: &SorobanEvent) -> Result<u64, anyhow::Error> {
        let ledger = match i64::try_from(event.ledger) {
            Ok(v) => v,
            Err(_) => {
                error!(
                    tx_hash = %event.tx_hash,
                    contract_id = %event.contract_id,
                    ledger = event.ledger,
                    event_type = %event.event_type,
                    "Ledger number overflows i64, skipping event",
                );
                return Ok(0);
            }
        };
        let timestamp = DateTime::parse_from_rfc3339(&event.ledger_closed_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| {
                warn!(
                    tx_hash = %event.tx_hash,
                    contract_id = %event.contract_id,
                    ledger = event.ledger,
                    event_type = %event.event_type,
                    raw = %event.ledger_closed_at,
                    "Unparseable ledger_closed_at, skipping event",
                );
                anyhow::anyhow!("Unparseable ledger_closed_at: {}", event.ledger_closed_at)
            })?;

        let event_data = json!({
            "value": event.value,
            "topic": event.topic
        });

        let result = sqlx::query(
            r#"
            INSERT INTO events (contract_id, event_type, tx_hash, ledger, timestamp, event_data)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (tx_hash, contract_id, event_type) DO NOTHING
            "#,
        )
        .bind(&event.contract_id)
        .bind(&event.event_type)
        .bind(&event.tx_hash)
        .bind(ledger)
        .bind(timestamp)
        .bind(event_data)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn make_event(ledger: u64) -> SorobanEvent {
        SorobanEvent {
            contract_id: "C1".into(),
            event_type: "contract".into(),
            tx_hash: "abc".into(),
            ledger,
            ledger_closed_at: "2026-03-24T00:00:00Z".into(),
            value: Value::Null,
            topic: None,
        }
    }

    #[test]
    fn ledger_overflow_returns_err() {
        assert!(i64::try_from(make_event(u64::MAX).ledger).is_err());
    }

    fn indexer(pool: PgPool) -> Indexer {
        Indexer {
            pool,
            client: Client::new(),
            config: Config {
                database_url: String::new(),
                stellar_rpc_url: String::new(),
                start_ledger: 0,
                port: 3000,
                behind_proxy: false,
            },
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn duplicate_insert_yields_one_row(pool: PgPool) {
        let indexer = indexer(pool.clone());
        let event = make_event(1);

        indexer.store_event(&event).await.unwrap();
        indexer.store_event(&event).await.unwrap(); // must not error

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn same_tx_hash_different_event_type_both_stored(pool: PgPool) {
        let indexer = indexer(pool.clone());
        let mut e1 = make_event(1);
        let mut e2 = make_event(1);
        e2.event_type = "system".into();

        indexer.store_event(&e1).await.unwrap();
        indexer.store_event(&e2).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
}
