//! Azure Event Hubs publisher integration.
//!
//! Configure `EVENT_HUBS_CONNECTION_STRING` and `EVENT_HUB_NAME`. Publishing
//! uses the Event Hubs HTTPS endpoint with a pooled reqwest client and SAS auth.

use crate::models::SorobanEvent;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::error;

type HmacSha256 = Hmac<Sha256>;

#[async_trait]
pub trait EventHubsPublisher: Send + Sync {
    async fn publish(&self, event: &SorobanEvent) -> Result<(), String>;
}

#[derive(Clone)]
pub struct HttpEventHubsPublisher {
    client: reqwest::Client,
    endpoint: String,
    hub_name: String,
    key_name: String,
    key: String,
    partition_strategy: String,
}

impl HttpEventHubsPublisher {
    pub fn from_connection_string(
        connection_string: &str,
        hub_name: String,
        partition_strategy: String,
    ) -> Result<Self, String> {
        let parts: HashMap<_, _> = connection_string
            .split(';')
            .filter_map(|part| part.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let endpoint = parts
            .get("Endpoint")
            .ok_or_else(|| "EVENT_HUBS_CONNECTION_STRING missing Endpoint".to_string())?
            .trim_start_matches("sb://")
            .trim_end_matches('/')
            .to_string();
        let key_name = parts
            .get("SharedAccessKeyName")
            .ok_or_else(|| "EVENT_HUBS_CONNECTION_STRING missing SharedAccessKeyName".to_string())?
            .to_string();
        let key = parts
            .get("SharedAccessKey")
            .ok_or_else(|| "EVENT_HUBS_CONNECTION_STRING missing SharedAccessKey".to_string())?
            .to_string();
        let hub_name = if hub_name.is_empty() {
            parts
                .get("EntityPath")
                .ok_or_else(|| "EVENT_HUB_NAME or EntityPath is required".to_string())?
                .to_string()
        } else {
            hub_name
        };

        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            .build()
            .map_err(|e| format!("failed to build Event Hubs HTTP client: {e}"))?;

        Ok(Self {
            client,
            endpoint,
            hub_name,
            key_name,
            key,
            partition_strategy,
        })
    }

    fn partition_key(&self, event: &SorobanEvent) -> String {
        match self.partition_strategy.as_str() {
            "tx_hash" => event.tx_hash.clone(),
            "ledger" => event.ledger.to_string(),
            _ => event.contract_id.clone(),
        }
    }

    fn sas_token(&self, resource_uri: &str) -> Result<String, String> {
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs()
            + 3600;
        let encoded_uri = encode_component(resource_uri);
        let string_to_sign = format!("{encoded_uri}\n{expiry}");
        let key = STANDARD
            .decode(&self.key)
            .map_err(|e| format!("invalid Event Hubs shared access key: {e}"))?;
        let mut mac = HmacSha256::new_from_slice(&key).map_err(|e| e.to_string())?;
        mac.update(string_to_sign.as_bytes());
        let signature = encode_component(&STANDARD.encode(mac.finalize().into_bytes()));
        Ok(format!(
            "SharedAccessSignature sr={encoded_uri}&sig={signature}&se={expiry}&skn={}",
            self.key_name
        ))
    }
}

#[async_trait]
impl EventHubsPublisher for HttpEventHubsPublisher {
    async fn publish(&self, event: &SorobanEvent) -> Result<(), String> {
        let resource_uri = format!("https://{}/{}", self.endpoint, self.hub_name);
        let url = format!("{resource_uri}/messages");
        let body = serde_json::to_vec(event)
            .map_err(|e| format!("failed to serialize Event Hubs event: {e}"))?;
        let response = self
            .client
            .post(url)
            .header("Authorization", self.sas_token(&resource_uri)?)
            .header("Content-Type", "application/json")
            .header("PartitionKey", self.partition_key(event))
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Event Hubs publish failed: {e}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Event Hubs publish returned {}", response.status()))
        }
    }
}

fn encode_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub async fn publish_event(publisher: &dyn EventHubsPublisher, event: &SorobanEvent) {
    if let Err(e) = publisher.publish(event).await {
        error!(error = %e, "failed to publish event to Event Hubs");
    }
}
