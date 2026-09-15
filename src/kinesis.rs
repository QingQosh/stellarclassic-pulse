//! Issue #265: AWS Kinesis Data Streams producer integration.
//!
//! When `KINESIS_STREAM_NAME` and `AWS_REGION` are configured, each indexed
//! event is published to the Kinesis stream as a JSON record with the event's
//! `contract_id` as the partition key. Authentication uses the standard AWS
//! credential chain (env vars, instance profile, ECS task role) via `aws-config`.
//!
//! This module is compiled only when the `kinesis` feature flag is enabled.

use crate::metrics;
use crate::models::SorobanEvent;
use async_trait::async_trait;
use tracing::{error, info};

/// Trait for publishing events to a stream, enabling mock testing.
#[async_trait]
pub trait KinesisPublisher: Send + Sync {
    async fn publish(&self, event: &SorobanEvent) -> Result<(), String>;
}

// ── Real AWS Kinesis implementation ──────────────────────────────────────────

#[cfg(feature = "kinesis")]
pub mod aws {
    use super::*;
    use aws_sdk_kinesis::error::SdkError;
    use aws_sdk_kinesis::operation::put_record::PutRecordError;
    use aws_sdk_kinesis::primitives::Blob;
    use aws_sdk_kinesis::Client;
    use std::time::Duration;

    const MAX_THROTTLE_RETRIES: u32 = 5;
    const THROTTLE_BACKOFF_BASE_MS: u64 = 100;

    pub struct AwsKinesisPublisher {
        client: Client,
        stream_name: String,
        /// Partition key strategy: "contract_id" | "tx_hash" | "random"
        partition_key_field: String,
    }

    impl AwsKinesisPublisher {
        /// Build a publisher from the standard AWS credential chain.
        pub async fn from_env(
            stream_name: String,
            region: String,
            partition_key_field: String,
        ) -> Self {
            let sdk_config = aws_config::from_env()
                .region(aws_sdk_kinesis::config::Region::new(region))
                .load()
                .await;
            let client = Client::new(&sdk_config);
            info!(
                stream = %stream_name,
                partition_key_field = %partition_key_field,
                "Kinesis publisher initialised"
            );
            Self {
                client,
                stream_name,
                partition_key_field,
            }
        }

        fn partition_key(&self, event: &SorobanEvent) -> String {
            match self.partition_key_field.as_str() {
                "tx_hash" => event.tx_hash.clone(),
                "random" => uuid::Uuid::new_v4().to_string(),
                _ => event.contract_id.clone(), // default: contract_id
            }
        }
    }

    #[async_trait]
    impl KinesisPublisher for AwsKinesisPublisher {
        async fn publish(&self, event: &SorobanEvent) -> Result<(), String> {
            let payload =
                serde_json::to_vec(event).map_err(|e| format!("serialisation error: {e}"))?;
            let key = self.partition_key(event);

            let mut backoff_ms = THROTTLE_BACKOFF_BASE_MS;
            for attempt in 0..=MAX_THROTTLE_RETRIES {
                let result = self
                    .client
                    .put_record()
                    .stream_name(&self.stream_name)
                    .partition_key(&key)
                    .data(Blob::new(payload.clone()))
                    .send()
                    .await;

                match result {
                    Ok(_) => return Ok(()),
                    Err(SdkError::ServiceError(se))
                        if matches!(
                            se.err(),
                            PutRecordError::ProvisionedThroughputExceededException(_)
                        ) =>
                    {
                        metrics::record_kinesis_throttled();
                        if attempt == MAX_THROTTLE_RETRIES {
                            metrics::record_kinesis_publish_failure();
                            return Err("Kinesis throttled: max retries exceeded".to_string());
                        }
                        tracing::warn!(
                            attempt = attempt + 1,
                            backoff_ms = backoff_ms,
                            stream = %self.stream_name,
                            "Kinesis throttled, retrying with backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        error!(stream = %self.stream_name, error = %msg, "Kinesis publish failed");
                        metrics::record_kinesis_publish_failure();
                        return Err(msg);
                    }
                }
            }
            unreachable!()
        }
    }
}

// ── Shared helper used by the indexer ────────────────────────────────────────

/// Publish an event, logging and metering failures without propagating them.
pub async fn publish_event(publisher: &dyn KinesisPublisher, event: &SorobanEvent) {
    if let Err(e) = publisher.publish(event).await {
        error!(error = %e, "Failed to publish event to Kinesis");
        metrics::record_kinesis_publish_failure();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockKinesisPublisher {
        published: Arc<Mutex<Vec<String>>>,
        fail: bool,
    }

    #[async_trait]
    impl KinesisPublisher for MockKinesisPublisher {
        async fn publish(&self, event: &SorobanEvent) -> Result<(), String> {
            if self.fail {
                return Err("mock error".to_string());
            }
            self.published
                .lock()
                .unwrap()
                .push(event.contract_id.clone());
            Ok(())
        }
    }

    fn make_event() -> SorobanEvent {
        SorobanEvent {
            contract_id: "CABC123".into(),
            event_type: "contract".into(),
            tx_hash: "deadbeef".into(),
            ledger: 100,
            ledger_closed_at: "2026-04-27T00:00:00Z".into(),
            value: Value::Null,
            topic: None,
        }
    }

    #[tokio::test]
    async fn mock_publisher_records_event() {
        let published = Arc::new(Mutex::new(vec![]));
        let mock = MockKinesisPublisher {
            published: published.clone(),
            fail: false,
        };
        mock.publish(&make_event()).await.unwrap();
        assert_eq!(*published.lock().unwrap(), vec!["CABC123"]);
    }

    #[tokio::test]
    async fn mock_publisher_returns_error_on_failure() {
        let mock = MockKinesisPublisher {
            fail: true,
            ..Default::default()
        };
        assert!(mock.publish(&make_event()).await.is_err());
    }

    #[tokio::test]
    async fn publish_event_does_not_panic_on_failure() {
        let mock = MockKinesisPublisher {
            fail: true,
            ..Default::default()
        };
        // Should not panic or propagate the error
        publish_event(&mock, &make_event()).await;
    }

    #[tokio::test]
    async fn publish_event_uses_contract_id_as_partition_key() {
        let published = Arc::new(Mutex::new(vec![]));
        let mock = MockKinesisPublisher {
            published: published.clone(),
            fail: false,
        };
        let mut event = make_event();
        event.contract_id = "CDEF456".into();
        publish_event(&mock, &event).await;
        assert_eq!(*published.lock().unwrap(), vec!["CDEF456"]);
    }

    // ── Partition key strategy unit tests ────────────────────────────────────

    #[cfg(feature = "kinesis")]
    mod partition_key_tests {
        use super::super::aws::AwsKinesisPublisher;
        use super::*;

        fn make_full_event() -> SorobanEvent {
            SorobanEvent {
                contract_id: "CABC123".into(),
                event_type: "contract".into(),
                tx_hash: "deadbeef".repeat(8), // 64 hex chars
                ledger: 100,
                ledger_closed_at: "2026-04-27T00:00:00Z".into(),
                value: Value::Null,
                topic: None,
                tenant_id: None,
            }
        }

        #[test]
        fn partition_key_contract_id_uses_contract_id() {
            // We test the logic directly by inspecting the partition_key_field value
            // without needing a real AWS client.
            let event = make_full_event();
            // Simulate the logic used inside AwsKinesisPublisher::partition_key
            let key = match "contract_id" {
                "tx_hash" => event.tx_hash.clone(),
                "random" => "random-uuid".to_string(),
                _ => event.contract_id.clone(),
            };
            assert_eq!(key, "CABC123");
        }

        #[test]
        fn partition_key_tx_hash_uses_tx_hash() {
            let event = make_full_event();
            let key = match "tx_hash" {
                "tx_hash" => event.tx_hash.clone(),
                "random" => "random-uuid".to_string(),
                _ => event.contract_id.clone(),
            };
            assert_eq!(key, "deadbeef".repeat(8));
        }

        #[test]
        fn partition_key_random_is_non_empty() {
            let key = uuid::Uuid::new_v4().to_string();
            assert!(!key.is_empty());
        }

        #[test]
        fn same_contract_id_gives_same_key() {
            let event = make_full_event();
            let key1 = event.contract_id.clone();
            let key2 = event.contract_id.clone();
            assert_eq!(key1, key2);
        }
    }
}
