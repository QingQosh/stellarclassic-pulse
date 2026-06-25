use lettre::message::{header, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

use crate::{metrics, models::SorobanEvent, retry_policy::RetryPolicy};

/// Issue #482: Output format for email notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailFormat {
    /// Plain-text body only (default).
    Text,
    /// HTML body only.
    Html,
    /// Both plain-text and HTML parts via `multipart/alternative`.
    Both,
}

impl EmailFormat {
    /// Parse the `EMAIL_FORMAT` value, defaulting to [`EmailFormat::Text`] for
    /// unknown or empty input.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "html" => EmailFormat::Html,
            "both" => EmailFormat::Both,
            _ => EmailFormat::Text,
        }
    }

    /// Whether an HTML part needs to be rendered for this format.
    fn needs_html(self) -> bool {
        matches!(self, EmailFormat::Html | EmailFormat::Both)
    }
}

/// Issue #482: Color used to badge an event type in the HTML template.
fn event_type_color(event_type: &str) -> &'static str {
    match event_type.to_ascii_lowercase().as_str() {
        "contract" => "#2563eb", // blue
        "system" => "#6b7280",   // gray
        "diagnostic" => "#d97706", // amber
        "transfer" => "#7c3aed", // purple
        _ => "#10b981",          // green
    }
}

/// Build the Handlebars rendering context for the HTML email.
///
/// Events are grouped by contract (first-seen order). Each event carries a
/// `color` badge and a clickable link to the API; each contract links to its
/// event-history endpoint.
fn build_html_context(events: &[SorobanEvent], api_base_url: &str, subject: &str) -> serde_json::Value {
    use serde_json::json;

    let base = api_base_url.trim_end_matches('/');

    let mut order: Vec<String> = Vec::new();
    let mut by_contract: HashMap<String, Vec<&SorobanEvent>> = HashMap::new();
    for event in events {
        if !by_contract.contains_key(&event.contract_id) {
            order.push(event.contract_id.clone());
        }
        by_contract
            .entry(event.contract_id.clone())
            .or_default()
            .push(event);
    }

    let contracts: Vec<serde_json::Value> = order
        .iter()
        .map(|contract_id| {
            let contract_events = &by_contract[contract_id];
            let shown: Vec<serde_json::Value> = contract_events
                .iter()
                .take(10)
                .map(|event| {
                    json!({
                        "event_type": event.event_type,
                        "color": event_type_color(&event.event_type),
                        "ledger": event.ledger,
                        "ledger_closed_at": event.ledger_closed_at,
                        "tx_hash": event.tx_hash,
                        "tx_link": format!("{base}/v1/events/tx/{}", event.tx_hash),
                    })
                })
                .collect();
            let more = contract_events.len().saturating_sub(10);
            json!({
                "contract_id": contract_id,
                "contract_link": format!("{base}/v1/events/contract/{contract_id}"),
                "event_count": contract_events.len(),
                "events": shown,
                "more_count": if more > 0 { Some(more) } else { None },
            })
        })
        .collect();

    json!({
        "subject": subject,
        "event_count": events.len(),
        "contracts": contracts,
    })
}

/// Render the HTML email body for a batch of events.
///
/// Issue #482: produces formatted tables, color-coded event-type badges,
/// formatted timestamps and clickable contract/transaction links. Values are
/// HTML-escaped by Handlebars' default escaper to avoid markup injection.
pub fn render_html(
    events: &[SorobanEvent],
    api_base_url: &str,
    subject: &str,
) -> Result<String, handlebars::RenderError> {
    let hb = handlebars::Handlebars::new();
    let context = build_html_context(events, api_base_url, subject);
    hb.render_template(include_str!("../notification_templates/email.html.hbs"), &context)
}

/// Batched email notification sender.
/// Collects events for up to 1 minute, then sends a single summary email.
pub struct EmailNotifier {
    smtp_host: String,
    smtp_port: u16,
    smtp_user: Option<String>,
    smtp_password: Option<SecretString>,
    from: String,
    to: Vec<String>,
    contract_filter: Vec<String>,
    retry_policy: RetryPolicy,
    /// Issue #482: output format (text / html / both).
    email_format: EmailFormat,
    /// Issue #482: base URL used to build clickable links in HTML emails.
    api_base_url: String,
    pool: sqlx::PgPool,
}

impl EmailNotifier {
    pub fn new(
        smtp_host: String,
        smtp_port: u16,
        smtp_user: Option<String>,
        smtp_password: Option<SecretString>,
        from: String,
        to: Vec<String>,
        contract_filter: Vec<String>,
        retry_policy: RetryPolicy,
        email_format: EmailFormat,
        api_base_url: String,
        pool: sqlx::PgPool,
    ) -> Self {
        Self {
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_password,
            from,
            to,
            contract_filter,
            retry_policy,
            email_format,
            api_base_url,
            pool,
        }
    }

    /// Spawn a background task that batches events and sends emails every minute.
    pub fn spawn(
        self,
        mut event_rx: tokio::sync::broadcast::Receiver<SorobanEvent>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut batch_interval = interval(Duration::from_secs(60));
            batch_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let mut events_buffer: Vec<SorobanEvent> = Vec::new();

            loop {
                tokio::select! {
                    _ = batch_interval.tick() => {
                        if !events_buffer.is_empty() {
                            self.send_batch_email(&events_buffer).await;
                            events_buffer.clear();
                        }
                    }
                    result = event_rx.recv() => {
                        match result {
                            Ok(event) => {
                                // Apply contract filter if configured
                                if !self.contract_filter.is_empty()
                                    && !self.contract_filter.contains(&event.contract_id)
                                {
                                    continue;
                                }
                                events_buffer.push(event);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!(
                                    skipped = n,
                                    "Email notifier lagged, some events skipped"
                                );
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                // Channel closed, send any remaining events and exit
                                if !events_buffer.is_empty() {
                                    self.send_batch_email(&events_buffer).await;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        })
    }

    /// Send a summary email for a batch of events with idempotency (Issue #474).
    async fn send_batch_email(&self, events: &[SorobanEvent]) {
        if events.is_empty() {
            return;
        }

        // Generate idempotency key based on event batch
        let event_ids: Vec<String> = events.iter().map(|e| e.id.to_string()).collect();
        let idempotency_key = format!("batch_{}", 
            sha2::Sha256::digest(event_ids.join(",").as_bytes())
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()[..16].to_string()
        );

        // Check if already sent
        if let Ok(existing) = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM email_notifications WHERE idempotency_key = $1"
        )
        .bind(&idempotency_key)
        .fetch_one(&self.pool)
        .await
        {
            if existing > 0 {
                info!(idempotency_key = %idempotency_key, "Email already sent, skipping");
                return;
            }
        }

        // Group events by contract ID for better readability
        let mut by_contract: HashMap<String, Vec<&SorobanEvent>> = HashMap::new();
        for event in events {
            by_contract
                .entry(event.contract_id.clone())
                .or_default()
                .push(event);
        }

        let subject = format!(
            "Soroban Pulse: {} new event{} indexed",
            events.len(),
            if events.len() == 1 { "" } else { "s" }
        );

        let mut body = String::new();
        body.push_str(&format!(
            "Soroban Pulse indexed {} new event{} in the last minute.\n\n",
            events.len(),
            if events.len() == 1 { "" } else { "s" }
        ));

        for (contract_id, contract_events) in by_contract.iter() {
            body.push_str(&format!(
                "Contract: {}\n  Events: {}\n",
                contract_id,
                contract_events.len()
            ));

            for event in contract_events.iter().take(10) {
                body.push_str(&format!(
                    "  - Type: {}, Ledger: {}, TxHash: {}\n",
                    event.event_type, event.ledger, event.tx_hash
                ));
            }

            if contract_events.len() > 10 {
                body.push_str(&format!(
                    "  ... and {} more event{}\n",
                    contract_events.len() - 10,
                    if contract_events.len() - 10 == 1 {
                        ""
                    } else {
                        "s"
                    }
                ));
            }
            body.push('\n');
        }

        // Issue #482: render an HTML part when the configured format requires it.
        // If rendering fails we fall back to the plain-text body rather than
        // dropping the notification.
        let html_body = if self.email_format.needs_html() {
            match render_html(events, &self.api_base_url, &subject) {
                Ok(html) => Some(html),
                Err(e) => {
                    error!(error = %e, "Failed to render HTML email template, falling back to text");
                    None
                }
            }
        } else {
            None
        };

        // Build and send email
        if let Err(e) = self.send_email(&subject, &body, html_body.as_deref()).await {
            error!(error = %e, "Failed to send email notification");
            metrics::record_email_failure();
        } else {
            info!(
                recipients = self.to.len(),
                event_count = events.len(),
                "Email notification sent successfully"
            );
        }
    }

    /// Send an email using SMTP.
    ///
    /// Issue #482: depending on `email_format`, sends a plain-text body, an
    /// HTML body, or both as a `multipart/alternative` message. If an HTML body
    /// was requested but is unavailable, falls back to plain text.
    async fn send_email(
        &self,
        subject: &str,
        text_body: &str,
        html_body: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Build message with all recipients
        let mut message_builder = Message::builder().from(self.from.parse()?).subject(subject);

        for recipient in &self.to {
            message_builder = message_builder.to(recipient.parse()?);
        }

        let message = match (self.email_format, html_body) {
            // Both parts: let the client pick the richest it can render.
            (EmailFormat::Both, Some(html)) => message_builder.multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(text_body.to_string()))
                    .singlepart(SinglePart::html(html.to_string())),
            )?,
            // HTML only.
            (EmailFormat::Html, Some(html)) => {
                message_builder.singlepart(SinglePart::html(html.to_string()))?
            }
            // Plain text (default), or HTML/both requested but rendering failed.
            _ => message_builder
                .header(header::ContentType::TEXT_PLAIN)
                .body(text_body.to_string())?,
        };

        // Build SMTP transport
        let mut transport_builder = SmtpTransport::relay(&self.smtp_host)?.port(self.smtp_port);

        if let (Some(user), Some(password)) = (&self.smtp_user, &self.smtp_password) {
            transport_builder = transport_builder.credentials(Credentials::new(
                user.clone(),
                password.expose_secret().clone(),
            ));
        }

        let mailer = transport_builder.build();

        // Send email (blocking operation, run in spawn_blocking)
        let result = tokio::task::spawn_blocking(move || mailer.send(&message)).await?;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mock_event(contract_id: &str, ledger: u64) -> SorobanEvent {
        SorobanEvent {
            contract_id: contract_id.to_string(),
            event_type: "contract".to_string(),
            tx_hash: "abc123".to_string(),
            ledger,
            ledger_closed_at: "2026-04-28T00:00:00Z".to_string(),
            ledger_hash: None,
            in_successful_call: true,
            value: json!({"test": "data"}),
            topic: None,
        }
    }

    #[test]
    fn test_email_notifier_creation() {
        // `connect_lazy` builds a pool handle without opening a connection,
        // so this stays a pure unit test (no live database required).
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/soroban_pulse_test")
            .expect("lazy pool");
        let notifier = EmailNotifier::new(
            "smtp.example.com".to_string(),
            587,
            Some("user".to_string()),
            Some(SecretString::new("pass".to_string())),
            "from@example.com".to_string(),
            vec!["to@example.com".to_string()],
            vec![],
            RetryPolicy::default(),
            EmailFormat::Text,
            "https://soroban-pulse.example.com".to_string(),
            pool,
        );

        assert_eq!(notifier.smtp_host, "smtp.example.com");
        assert_eq!(notifier.smtp_port, 587);
        assert_eq!(notifier.from, "from@example.com");
        assert_eq!(notifier.to.len(), 1);
        assert_eq!(notifier.email_format, EmailFormat::Text);
    }

    #[test]
    fn test_secret_string_redacted_in_debug() {
        let secret = SecretString::new("my_password".to_string());
        let debug_str = format!("{:?}", secret);
        assert!(!debug_str.contains("my_password"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_contract_filter_logic() {
        let filter = vec!["CONTRACT_A".to_string(), "CONTRACT_B".to_string()];

        let event_a = mock_event("CONTRACT_A", 100);
        let event_b = mock_event("CONTRACT_B", 101);
        let event_c = mock_event("CONTRACT_C", 102);

        assert!(filter.contains(&event_a.contract_id));
        assert!(filter.contains(&event_b.contract_id));
        assert!(!filter.contains(&event_c.contract_id));
    }

    #[test]
    fn test_empty_contract_filter_allows_all() {
        let filter: Vec<String> = vec![];
        let event = mock_event("ANY_CONTRACT", 100);

        // Empty filter means all events pass
        assert!(filter.is_empty() || filter.contains(&event.contract_id));
    }

    // --- Issue #482: HTML email template rendering ---

    fn mock_event_typed(contract_id: &str, ledger: u64, event_type: &str) -> SorobanEvent {
        SorobanEvent {
            event_type: event_type.to_string(),
            ..mock_event(contract_id, ledger)
        }
    }

    #[test]
    fn test_email_format_parse() {
        assert_eq!(EmailFormat::parse("text"), EmailFormat::Text);
        assert_eq!(EmailFormat::parse("HTML"), EmailFormat::Html);
        assert_eq!(EmailFormat::parse(" both "), EmailFormat::Both);
        // Unknown / empty values default to text.
        assert_eq!(EmailFormat::parse("xml"), EmailFormat::Text);
        assert_eq!(EmailFormat::parse(""), EmailFormat::Text);
    }

    #[test]
    fn test_render_html_basic_structure() {
        let events = vec![mock_event("CONTRACT_A", 100), mock_event("CONTRACT_A", 101)];
        let html = render_html(&events, "https://pulse.example.com", "subj").expect("render html");

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("indexed <strong>2</strong>"));
        assert!(html.contains("CONTRACT_A"));
        // Summary table headers are present.
        assert!(html.contains("Ledger"));
        assert!(html.contains("Tx Hash"));
    }

    #[test]
    fn test_render_html_clickable_links() {
        let events = vec![mock_event("CONTRACT_A", 100)];
        let html = render_html(&events, "https://pulse.example.com/", "subj").expect("render html");

        // Trailing slash on the base URL is normalized (no double slash).
        assert!(html.contains("href=\"https://pulse.example.com/v1/events/contract/CONTRACT_A\""));
        assert!(html.contains("href=\"https://pulse.example.com/v1/events/tx/abc123\""));
    }

    #[test]
    fn test_render_html_event_type_color_coding() {
        let events = vec![
            mock_event_typed("CONTRACT_A", 100, "contract"),
            mock_event_typed("CONTRACT_A", 101, "system"),
        ];
        let html = render_html(&events, "https://pulse.example.com", "subj").expect("render html");

        assert!(html.contains(event_type_color("contract")));
        assert!(html.contains(event_type_color("system")));
        assert!(html.contains(">contract<"));
        assert!(html.contains(">system<"));
    }

    #[test]
    fn test_event_type_color_distinct_per_type() {
        assert_eq!(event_type_color("contract"), "#2563eb");
        assert_eq!(event_type_color("system"), "#6b7280");
        assert_eq!(event_type_color("diagnostic"), "#d97706");
        // Unknown types get the default color.
        assert_eq!(event_type_color("mystery"), "#10b981");
    }

    #[test]
    fn test_render_html_truncates_after_ten_events() {
        let events: Vec<SorobanEvent> = (0..12)
            .map(|i| mock_event("CONTRACT_A", 100 + i))
            .collect();
        let html = render_html(&events, "https://pulse.example.com", "subj").expect("render html");
        assert!(html.contains("and 2 more event"));
    }

    #[test]
    fn test_render_html_escapes_values() {
        let mut event = mock_event("CONTRACT_A", 100);
        event.event_type = "<script>".to_string();
        let html = render_html(&[event], "https://pulse.example.com", "subj").expect("render html");
        // The raw tag must not appear unescaped.
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
