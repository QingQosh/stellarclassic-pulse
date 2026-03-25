use chrono::DateTime;
use reqwest::Client;
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{
    config::Config,
    models::{GetEventsResult, LatestLedgerResult, RpcResponse, SorobanEvent},
};

#[derive(Debug, thiserror::Error)]
enum IndexerFetchError {
    #[error("{0}")]
    Rpc(String),
    #[error(transparent)]
    DbConnection(#[from] sqlx::Error),
}


pub struct Indexer {
    pool: PgPool,
    client: Client,
    config: Config,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
}

impl Indexer {
    pub fn new(pool: PgPool, config: Config, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> Self {
        Self {
            pool,
            client: Client::new(),
            config,
            shutdown_rx,
        }
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
                        info!("Starting from latest ledger: {}", current_ledger);
                        break;
                    }
                    Err(e) => {
                        error!("Failed to get latest ledger (attempt {}): {}", retries + 1, e);
                        retries += 1;
                        if retries >= 5 {
                            if self.config.start_ledger_fallback {
                                warn!("Falling back to genesis ledger (1) due to RPC failure");
                                current_ledger = 1;
                                break;
                            } else {
                                error!("Fatal RPC error: Could not fetch initial ledger after 5 attempts.");
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
                    if latest > current_ledger {
                        current_ledger = latest;
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
                        error!("Indexer error: {e}");
                    }
                    sleep(Duration::from_secs(backoff_secs)).await;
                }
                Err(IndexerFetchError::Rpc(msg)) => {
                    error!("Indexer error: {}", msg);
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

        let resp: RpcResponse<LatestLedgerResult> = self
            .client
            .post(&self.config.stellar_rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        match resp.result {
            Some(r) => Ok(r.sequence),
            None => {
                if let Some(err) = resp.error {
                    warn!("RPC error response: {}", err.message);
                }
                Err("RPC returned no result".to_string())
            }
        }
    }

    async fn fetch_and_store_events(&self, start_ledger: u64) -> Result<u64, IndexerFetchError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getEvents",
            "params": {
                "startLedger": start_ledger,
                "filters": [],
                "pagination": { "limit": 100 }
            }
        });

        let resp: RpcResponse<GetEventsResult> = self
            .client
            .post(&self.config.stellar_rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerFetchError::Rpc(e.to_string()))?
            .json()
            .await
            .map_err(|e| IndexerFetchError::Rpc(e.to_string()))?;

        let result = match resp.result {
            Some(r) => r,
            None => {
                if let Some(err) = resp.error {
                    warn!("RPC error response: {}", err.message);
                }
                return Err(IndexerFetchError::Rpc("RPC returned no result".to_string()));
            }
        };

        let latest = result.latest_ledger;
        let total = result.events.len();
        let mut new = 0;
        let mut skipped = 0;

        for event in result.events {
            match self.store_event(&event).await {
                Ok(rows) => {
                    new += rows;
                    if rows == 0 {
                        skipped += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to store event {}: {}", event.tx_hash, e);
                }
            }
        }

        info!(
            fetched = total,
            inserted = new,
            ledger = latest,
            "Indexed ledger range"
        );

        // TODO(#42): Add a duplicate_events_skipped counter to the future metrics endpoint
        let _duplicate_events_skipped = skipped;

        Ok(latest + 1)
    }
    async fn store_event(&self, event: &SorobanEvent) -> Result<u64, anyhow::Error> {
        let ledger = match i64::try_from(event.ledger) {
            Ok(v) => v,
            Err(_) => {
                error!(ledger = event.ledger, "Ledger number overflows i64, skipping event");
                return Ok(0);
            }
        };
        let timestamp = DateTime::parse_from_rfc3339(&event.ledger_closed_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| {
                warn!(raw = event.ledger_closed_at, "Unparseable ledger_closed_at, skipping event");
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

    #[test]
    fn test_rpc_error_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: RpcResponse<GetEventsResult> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().message, "Invalid Request");
    }

    #[test]
    fn test_rpc_success_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"events":[],"latestLedger":123}}"#;
        let resp: RpcResponse<GetEventsResult> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap().latest_ledger, 123);
    }

    #[tokio::test]
    async fn test_store_event_malformed_timestamp() {
        let pool = PgPool::connect_lazy("postgres://localhost/unused").unwrap();
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let indexer = Indexer::new(pool, Config::default(), shutdown_rx);
        let mut event = make_event(100);
        event.ledger_closed_at = "invalid-date".into();

        let result = indexer.store_event(&event).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unparseable ledger_closed_at: invalid-date"));
    }
}
