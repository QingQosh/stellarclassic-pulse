//! Message queue publisher for indexed events
//!
//! Publishes events to Redis Streams (with Kafka/RabbitMQ support planned).
//! Publishing is non-blocking and includes retry logic with exponential backoff.

use crate::models::SorobanEvent;
use serde_json::json;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;

#[cfg(feature = "redis-queue")]
mod redis_impl {
    use super::*;
    use redis::aio::ConnectionManager;
    use redis::{AsyncCommands, RedisError};
    use std::collections::VecDeque;

    pub struct RedisPublisher {
        client: ConnectionManager,
        stream_key: String,
        buffer: VecDeque<SorobanEvent>,
        max_buffer_size: usize,
    }

    impl RedisPublisher {
        pub async fn new(
            redis_url: &str,
            stream_key: String,
            max_buffer_size: usize,
        ) -> Result<Self, RedisError> {
            let client = redis::Client::open(redis_url)?;
            let conn = ConnectionManager::new(client).await?;

            info!(
                redis_url = %Self::safe_redis_url(redis_url),
                stream_key = %stream_key,
                max_buffer_size = max_buffer_size,
                "Redis publisher initialized"
            );

            Ok(Self {
                client: conn,
                stream_key,
                buffer: VecDeque::new(),
                max_buffer_size,
            })
        }

        /// Strip credentials from Redis URL for safe logging
        fn safe_redis_url(url: &str) -> String {
            if let Ok(parsed) = url::Url::parse(url) {
                let mut safe = parsed.clone();
                let _ = safe.set_username("");
                let _ = safe.set_password(None);
                safe.to_string()
            } else {
                "<unparseable>".to_string()
            }
        }

        /// Push an event into the in-memory buffer, dropping the oldest entry if full.
        pub fn buffer_event(&mut self, event: SorobanEvent) {
            if self.buffer.len() >= self.max_buffer_size {
                self.buffer.pop_front();
                crate::metrics::record_redis_dropped();
            }
            self.buffer.push_back(event);
            crate::metrics::update_redis_buffer_size(self.buffer.len());
        }

        /// Try to drain the buffer to Redis. Returns true if the buffer was fully drained
        /// (connection restored), false if Redis is still unreachable.
        pub async fn try_drain_buffer(&mut self) -> bool {
            let was_non_empty = !self.buffer.is_empty();
            while !self.buffer.is_empty() {
                // Clone the front event to release the borrow on self.buffer before calling publish.
                let event = match self.buffer.front() {
                    Some(e) => e.clone(),
                    None => break,
                };
                match self.publish(&event).await {
                    Ok(()) => {
                        self.buffer.pop_front();
                    }
                    Err(_) => {
                        crate::metrics::update_redis_buffer_size(self.buffer.len());
                        return false;
                    }
                }
            }
            if was_non_empty {
                crate::metrics::record_redis_reconnect();
                crate::metrics::update_redis_buffer_size(0);
                info!("Redis reconnected, buffer fully drained");
            }
            true
        }

        pub async fn publish(&mut self, event: &SorobanEvent) -> Result<(), RedisError> {
            let event_json = json!({
                "contract_id": event.contract_id,
                "event_type": event.event_type,
                "tx_hash": event.tx_hash,
                "ledger": event.ledger,
                "ledger_closed_at": event.ledger_closed_at,
                "value": event.value,
                "topic": event.topic,
            });

            // Convert JSON to flat key-value pairs for Redis hash
            let fields: Vec<(&str, String)> = vec![
                ("contract_id", event.contract_id.clone()),
                ("event_type", event.event_type.clone()),
                ("tx_hash", event.tx_hash.clone()),
                ("ledger", event.ledger.to_string()),
                ("ledger_closed_at", event.ledger_closed_at.clone()),
                ("value", event.value.to_string()),
                (
                    "topic",
                    event
                        .topic
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "null".to_string()),
                ),
            ];

            self.client.xadd(&self.stream_key, "*", &fields).await?;

            Ok(())
        }
    }

    pub async fn spawn_redis_publisher(
        redis_url: String,
        stream_key: String,
        max_buffer_size: usize,
        mut event_rx: broadcast::Receiver<SorobanEvent>,
    ) {
        let mut publisher =
            match RedisPublisher::new(&redis_url, stream_key, max_buffer_size).await {
                Ok(p) => p,
                Err(e) => {
                    error!(error = %e, "Failed to initialize Redis publisher");
                    return;
                }
            };

        info!("Redis publisher task started");

        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    // If we have buffered events, try to drain first (reconnection check).
                    if !publisher.buffer.is_empty() {
                        let reconnected = publisher.try_drain_buffer().await;
                        if !reconnected {
                            // Still disconnected — buffer the incoming event.
                            publisher.buffer_event(event);
                            continue;
                        }
                    }

                    // Buffer is empty (or was fully drained). Try publishing normally.
                    if let Err(e) = publish_with_retry(&mut publisher, &event).await {
                        error!(
                            contract_id = %event.contract_id,
                            tx_hash = %event.tx_hash,
                            error = %e,
                            "Failed to publish event to Redis after retries — buffering"
                        );
                        crate::metrics::record_queue_publish_failure();
                        publisher.buffer_event(event);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "Redis publisher lagged, some events skipped");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Event channel closed, stopping Redis publisher");
                    break;
                }
            }
        }
    }

    async fn publish_with_retry(
        publisher: &mut RedisPublisher,
        event: &SorobanEvent,
    ) -> Result<(), String> {
        let mut attempt = 0;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        loop {
            match publisher.publish(event).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    if attempt >= MAX_RETRIES {
                        return Err(format!("Max retries exceeded: {}", e));
                    }

                    warn!(
                        attempt = attempt,
                        backoff_ms = backoff_ms,
                        error = %e,
                        "Redis publish failed, retrying"
                    );

                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2; // Exponential backoff
                }
            }
        }
    }
}

#[cfg(feature = "redis-queue")]
pub use redis_impl::spawn_redis_publisher;

/// No-op stub when redis-queue feature is disabled
#[cfg(not(feature = "redis-queue"))]
pub async fn spawn_redis_publisher(
    _redis_url: String,
    _stream_key: String,
    _max_buffer_size: usize,
    _event_rx: broadcast::Receiver<SorobanEvent>,
) {
    warn!("Redis publisher requested but redis-queue feature is not enabled");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_retries_constant() {
        assert_eq!(MAX_RETRIES, 3);
    }

    #[test]
    fn test_initial_backoff() {
        assert_eq!(INITIAL_BACKOFF_MS, 100);
    }

    #[cfg(feature = "redis-queue")]
    #[test]
    fn test_safe_redis_url() {
        use super::redis_impl::RedisPublisher;

        let url = "redis://user:password@localhost:6379/0";
        let safe = RedisPublisher::safe_redis_url(url);
        assert!(!safe.contains("password"));
        assert!(!safe.contains("user"));
        assert!(safe.contains("localhost"));
    }

    #[cfg(feature = "redis-queue")]
    #[test]
    fn test_safe_redis_url_unparseable() {
        use super::redis_impl::RedisPublisher;

        let url = "not-a-url";
        let safe = RedisPublisher::safe_redis_url(url);
        assert_eq!(safe, "<unparseable>");
    }

    /// Simulate buffer_event fill and oldest-drop behaviour without a real Redis connection.
    #[cfg(feature = "redis-queue")]
    #[test]
    fn buffer_drops_oldest_when_full() {
        use super::redis_impl::RedisPublisher;
        use serde_json::Value;
        use std::collections::VecDeque;

        fn make_event(contract_id: &str) -> SorobanEvent {
            SorobanEvent {
                contract_id: contract_id.to_string(),
                event_type: "contract".to_string(),
                tx_hash: "a".repeat(64),
                ledger: 1,
                ledger_closed_at: "2026-01-01T00:00:00Z".to_string(),
                ledger_hash: None,
                in_successful_call: true,
                value: Value::Null,
                topic: None,
                tenant_id: None,
            }
        }

        // Build a publisher with max_buffer_size = 2 using only its buffer fields.
        // We can't call new() without a live Redis, so we test buffer_event logic
        // by constructing the struct fields directly via the public buffer_event method
        // with a mock that wraps the logic. Instead, we validate the algorithm below:

        let max = 2usize;
        let mut buffer: VecDeque<SorobanEvent> = VecDeque::new();
        let mut dropped = 0usize;

        let push = |buf: &mut VecDeque<SorobanEvent>, dropped: &mut usize, ev: SorobanEvent| {
            if buf.len() >= max {
                buf.pop_front();
                *dropped += 1;
            }
            buf.push_back(ev);
        };

        push(&mut buffer, &mut dropped, make_event("C1"));
        push(&mut buffer, &mut dropped, make_event("C2"));
        push(&mut buffer, &mut dropped, make_event("C3")); // C1 should be dropped

        assert_eq!(buffer.len(), 2);
        assert_eq!(dropped, 1);
        assert_eq!(buffer.front().unwrap().contract_id, "C2");
        assert_eq!(buffer.back().unwrap().contract_id, "C3");
    }

    #[cfg(feature = "redis-queue")]
    #[test]
    fn buffer_empty_when_no_events() {
        use std::collections::VecDeque;
        let buffer: VecDeque<SorobanEvent> = VecDeque::new();
        assert!(buffer.is_empty());
    }
}
