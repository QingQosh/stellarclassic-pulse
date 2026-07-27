use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{metrics, models::SorobanEvent};

/// Discord webhook configuration
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    pub webhook_url: String,
    pub bot_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Discord API client for webhook-based notifications
pub struct DiscordClient {
    client: Client,
    webhook_url: String,
    bot_name: Option<String>,
    avatar_url: Option<String>,
}

impl DiscordClient {
    pub fn new(config: DiscordConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build Discord HTTP client");

        Self {
            client,
            webhook_url: config.webhook_url,
            bot_name: config.bot_name,
            avatar_url: config.avatar_url,
        }
    }

    /// Send a formatted message to Discord using embeds
    pub async fn send_event_notification(
        &self,
        event: &SorobanEvent,
        thread_id: Option<String>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let color = match event.event_type.as_str() {
            "contract" => 3447003, // Blue
            "diagnostic" => 10181046, // Orange
            "system" => 15158332, // Red
            _ => 9807270, // Gray
        };

        let mut embed = json!({
            "title": format!("Soroban Event: {}", event.event_type),
            "description": format!("Contract: {}", event.contract_id),
            "color": color,
            "fields": [
                {
                    "name": "Transaction Hash",
                    "value": format!("`{}`", event.tx_hash),
                    "inline": true
                },
                {
                    "name": "Ledger",
                    "value": event.ledger.to_string(),
                    "inline": true
                },
                {
                    "name": "Timestamp",
                    "value": event.timestamp.to_string(),
                    "inline": false
                },
                {
                    "name": "Event Type",
                    "value": event.event_type.to_string(),
                    "inline": true
                }
            ],
            "footer": {
                "text": "Soroban Pulse"
            }
        });

        // Add event data as a code block if available
        if !event.event_data.is_empty() {
            if let Ok(data_str) = serde_json::to_string_pretty(&event.event_data) {
                embed["fields"].as_array_mut().map(|fields| {
                    fields.push(json!({
                        "name": "Event Data",
                        "value": format!("```json\n{}\n```", data_str),
                        "inline": false
                    }))
                });
            }
        }

        let mut payload = json!({
            "embeds": [embed]
        });

        if let Some(name) = &self.bot_name {
            payload["username"] = json!(name);
        }

        if let Some(avatar) = &self.avatar_url {
            payload["avatar_url"] = json!(avatar);
        }

        // Add thread ID if specified
        if let Some(thread) = thread_id {
            payload["thread_id"] = json!(thread);
        }

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let message_id = Uuid::new_v4().to_string();
            info!(
                webhook_url = %self.webhook_url,
                message_id = %message_id,
                contract_id = %event.contract_id,
                "Discord notification sent"
            );

            Ok(message_id)
        } else {
            let error_body = response.text().await.unwrap_or_default();
            error!(
                status = %response.status(),
                body = %error_body,
                "Failed to send Discord notification"
            );
            Err(format!("Discord API error: {}", error_body).into())
        }
    }

    /// Send a simple text message to Discord
    pub async fn send_message(&self, content: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "content": content
        });

        if let Some(name) = &self.bot_name {
            payload["username"] = json!(name);
        }

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let message_id = Uuid::new_v4().to_string();
            info!(
                webhook_url = %self.webhook_url,
                message_id = %message_id,
                "Discord message sent"
            );

            Ok(message_id)
        } else {
            let error_body = response.text().await.unwrap_or_default();
            error!(
                status = %response.status(),
                body = %error_body,
                "Failed to send Discord message"
            );
            Err(format!("Discord API error: {}", error_body).into())
        }
    }

    /// Send event to Discord with retry logic
    pub async fn send_with_retry(
        &self,
        event: &SorobanEvent,
        max_retries: u32,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut backoff_ms = 1000u64;

        for attempt in 1..=max_retries {
            match self.send_event_notification(event, None).await {
                Ok(message_id) => {
                    info!(attempt = attempt, message_id = %message_id, "Discord notification sent successfully");
                    return Ok(message_id);
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        attempt = attempt,
                        "Discord API request failed"
                    );
                }
            }

            if attempt < max_retries {
                sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms *= 2;
            }
        }

        metrics::record_discord_failure();
        Err("Discord delivery failed after retries".into())
    }

    /// Update an existing message in Discord
    pub async fn update_message(
        &self,
        message_id: &str,
        content: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Note: Discord webhooks don't support message editing directly
        // This would require using the Discord API with a bot token instead
        Err("Message editing not supported via webhooks".into())
    }
}

/// Deliver an event to Discord with retry logic
pub async fn deliver_discord(
    client: &DiscordClient,
    event: SorobanEvent,
) {
    if let Err(e) = client.send_with_retry(&event, 3).await {
        error!(
            error = %e,
            contract_id = %event.contract_id,
            event_type = %event.event_type,
            "Failed to deliver Discord notification"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_client_creation() {
        let config = DiscordConfig {
            webhook_url: "https://discord.com/api/webhooks/test".to_string(),
            bot_name: Some("Soroban Bot".to_string()),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
        };

        let client = DiscordClient::new(config);
        assert_eq!(client.webhook_url, "https://discord.com/api/webhooks/test");
        assert_eq!(client.bot_name, Some("Soroban Bot".to_string()));
    }

    #[test]
    fn test_discord_config_creation() {
        let config = DiscordConfig {
            webhook_url: "https://discord.com/api/webhooks/test".to_string(),
            bot_name: None,
            avatar_url: None,
        };

        assert_eq!(config.webhook_url, "https://discord.com/api/webhooks/test");
        assert!(config.bot_name.is_none());
    }
}
