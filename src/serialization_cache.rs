use serde_json::Value;
use std::sync::Arc;
use moka::future::Cache;
use std::time::Instant;

pub struct SerializationMetrics {
    pub total_serializations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_serialization_time_us: u64,
}

pub struct SerializedEventCache {
    cache: Arc<Cache<String, Vec<u8>>>,
    metrics: Arc<tokio::sync::Mutex<SerializationMetrics>>,
}

impl SerializedEventCache {
    pub fn new(max_capacity: u64, ttl_secs: u64) -> Self {
        let cache = Arc::new(
            Cache::builder()
                .max_capacity(max_capacity)
                .time_to_live(std::time::Duration::from_secs(ttl_secs))
                .build(),
        );

        let metrics = Arc::new(tokio::sync::Mutex::new(SerializationMetrics {
            total_serializations: 0,
            cache_hits: 0,
            cache_misses: 0,
            total_serialization_time_us: 0,
        }));

        Self { cache, metrics }
    }

    pub async fn get_or_serialize<F>(
        &self,
        key: &str,
        value: &Value,
        fallback: F,
    ) -> Result<Vec<u8>, serde_json::Error>
    where
        F: FnOnce(&Value) -> Result<Vec<u8>, serde_json::Error>,
    {
        if let Some(cached) = self.cache.get(key).await {
            let mut metrics = self.metrics.lock().await;
            metrics.cache_hits += 1;
            crate::metrics::record_serialization_cache_hit("event");
            return Ok(cached);
        }

        let start = Instant::now();
        let serialized = fallback(value)?;
        let duration = start.elapsed();

        self.cache
            .insert(key.to_string(), serialized.clone())
            .await;

        let duration_us = duration.as_micros() as u64;
        let mut metrics = self.metrics.lock().await;
        metrics.cache_misses += 1;
        metrics.total_serializations += 1;
        metrics.total_serialization_time_us += duration_us;

        crate::metrics::record_serialization_time("event", duration_us);
        crate::metrics::record_serialization_cache_miss("event");

        Ok(serialized)
    }

    pub async fn get_metrics(&self) -> SerializationMetrics {
        let metrics = self.metrics.lock().await;
        SerializationMetrics {
            total_serializations: metrics.total_serializations,
            cache_hits: metrics.cache_hits,
            cache_misses: metrics.cache_misses,
            total_serialization_time_us: metrics.total_serialization_time_us,
        }
    }

    pub async fn clear(&self) {
        self.cache.invalidate_all();
        let mut metrics = self.metrics.lock().await;
        metrics.total_serializations = 0;
        metrics.cache_hits = 0;
        metrics.cache_misses = 0;
        metrics.total_serialization_time_us = 0;
    }
}

pub fn optimize_serialization(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(value)
}

pub fn optimize_serialization_pretty(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

pub fn serialize_compact(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut buffer = Vec::with_capacity(1024);
    let mut ser = serde_json::Serializer::new(&mut buffer);
    value.serialize(&mut ser)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn serialization_cache_basic() {
        let cache = SerializedEventCache::new(100, 300);
        let event = json!({
            "type": "contract_event",
            "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABC",
            "data": {"key": "value"}
        });

        let key = "event:12345";
        let result = cache
            .get_or_serialize(key, &event, |v| serde_json::to_vec(v))
            .await;

        assert!(result.is_ok());

        // Second call should hit cache
        let result2 = cache
            .get_or_serialize(key, &event, |v| serde_json::to_vec(v))
            .await;

        assert!(result2.is_ok());
        assert_eq!(result.unwrap(), result2.unwrap());

        let metrics = cache.get_metrics().await;
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);
    }

    #[test]
    fn optimize_serialization_basic() {
        let event = json!({
            "type": "contract_event",
            "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABC"
        });

        let result = optimize_serialization(&event);
        assert!(result.is_ok());

        let serialized = result.unwrap();
        assert!(!serialized.is_empty());
    }

    #[test]
    fn serialize_compact_efficiency() {
        let large_event = json!({
            "type": "contract_event",
            "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABC",
            "data": {
                "field1": "value1",
                "field2": "value2",
                "field3": "value3",
                "nested": {
                    "inner1": "data1",
                    "inner2": "data2"
                }
            }
        });

        let result = serialize_compact(&large_event);
        assert!(result.is_ok());

        let serialized = result.unwrap();
        assert!(!serialized.is_empty());
    }
}
