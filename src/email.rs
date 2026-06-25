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

/// Issue #480: Languages with a bundled email notification template.
///
/// Each entry corresponds to a Handlebars file stored under the
/// `notification_templates/` directory (e.g. `email_en.hbs`).
pub const SUPPORTED_LANGUAGES: &[&str] = &["en", "es", "zh", "ja"];

/// Normalize a user-supplied language code to one we have a template for.
///
/// Unknown or empty values fall back to English so notifications are never
/// dropped just because of an unrecognized language setting.
pub fn normalize_language(language: &str) -> &'static str {
    match language.trim().to_ascii_lowercase().as_str() {
        "es" => "es",
        "zh" => "zh",
        "ja" => "ja",
        _ => "en",
    }
}

/// Return the raw Handlebars template source for a normalized language.
///
/// Templates are embedded at compile time so they ship with the binary and do
/// not depend on the process working directory at runtime.
fn template_source(language: &str) -> &'static str {
    match normalize_language(language) {
        "es" => include_str!("../notification_templates/email_es.hbs"),
        "zh" => include_str!("../notification_templates/email_zh.hbs"),
        "ja" => include_str!("../notification_templates/email_ja.hbs"),
        _ => include_str!("../notification_templates/email_en.hbs"),
    }
}

/// Build the localized email subject line for a batch of `count` events.
pub fn localized_subject(language: &str, count: usize) -> String {
    match normalize_language(language) {
        "es" => format!("Soroban Pulse: {} nuevo(s) evento(s) indexado(s)", count),
        "zh" => format!("Soroban Pulse：已索引 {} 个新事件", count),
        "ja" => format!(
            "Soroban Pulse: {} 件の新しいイベントをインデックスしました",
            count
        ),
        _ => format!(
            "Soroban Pulse: {} new event{} indexed",
            count,
            if count == 1 { "" } else { "s" }
        ),
    }
}

/// Build the Handlebars rendering context for a batch of events.
///
/// Events are grouped by contract (preserving first-seen order) and each group
/// exposes up to 10 events plus a `more_count` for the remainder.
fn build_context(events: &[SorobanEvent]) -> serde_json::Value {
    use serde_json::json;

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
                        "ledger": event.ledger,
                        "tx_hash": event.tx_hash,
                        "ledger_closed_at": event.ledger_closed_at,
                    })
                })
                .collect();
            let more = contract_events.len().saturating_sub(10);
            json!({
                "contract_id": contract_id,
                "event_count": contract_events.len(),
                "events": shown,
                "more_count": if more > 0 { Some(more) } else { None },
            })
        })
        .collect();

    json!({
        "event_count": events.len(),
        "contracts": contracts,
    })
}

/// Render the localized plain-text email body for a batch of events.
///
/// Issue #480: the template is selected from [`SUPPORTED_LANGUAGES`]; unknown
/// languages fall back to English via [`normalize_language`].
pub fn render_body(
    language: &str,
    events: &[SorobanEvent],
) -> Result<String, handlebars::RenderError> {
    let mut hb = handlebars::Handlebars::new();
    // Plain-text output: don't HTML-escape contract IDs, hashes, etc.
    hb.register_escape_fn(handlebars::no_escape);
    let context = build_context(events);
    hb.render_template(template_source(language), &context)
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
    /// Issue #480: language used to render notification templates (default `en`).
    language: String,
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
        language: String,
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
            language,
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

        // Issue #480: render the summary in the configured language using a
        // Handlebars template stored under `notification_templates/`.
        let subject = localized_subject(&self.language, events.len());

        let body = match render_body(&self.language, events) {
            Ok(body) => body,
            Err(e) => {
                error!(error = %e, language = %self.language, "Failed to render email template");
                metrics::record_email_failure();
                return;
            }
        };

        // Build and send email
        if let Err(e) = self.send_email(&subject, &body).await {
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
    async fn send_email(
        &self,
        subject: &str,
        body: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Build message with all recipients
        let mut message_builder = Message::builder().from(self.from.parse()?).subject(subject);

        for recipient in &self.to {
            message_builder = message_builder.to(recipient.parse()?);
        }

        let message = message_builder
            .header(header::ContentType::TEXT_PLAIN)
            .body(body.to_string())?;

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
            "en".to_string(),
            pool,
        );

        assert_eq!(notifier.smtp_host, "smtp.example.com");
        assert_eq!(notifier.smtp_port, 587);
        assert_eq!(notifier.from, "from@example.com");
        assert_eq!(notifier.to.len(), 1);
        assert_eq!(notifier.language, "en");
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

    // --- Issue #480: multi-language template rendering ---

    #[test]
    fn test_normalize_language_falls_back_to_en() {
        assert_eq!(normalize_language("en"), "en");
        assert_eq!(normalize_language("ES"), "es");
        assert_eq!(normalize_language(" zh "), "zh");
        assert_eq!(normalize_language("ja"), "ja");
        // Unknown / empty languages fall back to English.
        assert_eq!(normalize_language("fr"), "en");
        assert_eq!(normalize_language(""), "en");
    }

    #[test]
    fn test_render_body_english() {
        let events = vec![mock_event("CONTRACT_A", 100), mock_event("CONTRACT_A", 101)];
        let body = render_body("en", &events).expect("render en");
        assert!(body.contains("indexed 2 new event"));
        assert!(body.contains("Contract: CONTRACT_A"));
        assert!(body.contains("Type: contract"));
        assert!(body.contains("TxHash: abc123"));
    }

    #[test]
    fn test_render_body_spanish() {
        let events = vec![mock_event("CONTRACT_A", 100)];
        let body = render_body("es", &events).expect("render es");
        assert!(body.contains("indexó 1"));
        assert!(body.contains("Contrato: CONTRACT_A"));
        assert!(body.contains("Tipo: contract"));
    }

    #[test]
    fn test_render_body_chinese() {
        let events = vec![mock_event("CONTRACT_A", 100)];
        let body = render_body("zh", &events).expect("render zh");
        assert!(body.contains("个新事件"));
        assert!(body.contains("合约：CONTRACT_A"));
        assert!(body.contains("类型：contract"));
    }

    #[test]
    fn test_render_body_japanese() {
        let events = vec![mock_event("CONTRACT_A", 100)];
        let body = render_body("ja", &events).expect("render ja");
        assert!(body.contains("件の新しいイベント"));
        assert!(body.contains("コントラクト: CONTRACT_A"));
        assert!(body.contains("種類: contract"));
    }

    #[test]
    fn test_render_body_unknown_language_uses_english() {
        let events = vec![mock_event("CONTRACT_A", 100)];
        let body = render_body("fr", &events).expect("render fallback");
        assert!(body.contains("indexed 1 new event"));
    }

    #[test]
    fn test_render_body_truncates_after_ten_events() {
        let events: Vec<SorobanEvent> = (0..13)
            .map(|i| mock_event("CONTRACT_A", 100 + i))
            .collect();
        let body = render_body("en", &events).expect("render truncated");
        assert!(body.contains("... and 3 more event"));
    }

    #[test]
    fn test_all_supported_languages_render() {
        let events = vec![mock_event("CONTRACT_A", 100)];
        for lang in SUPPORTED_LANGUAGES {
            let body = render_body(lang, &events)
                .unwrap_or_else(|e| panic!("render {lang} failed: {e}"));
            assert!(!body.trim().is_empty(), "empty body for {lang}");
        }
        assert_eq!(SUPPORTED_LANGUAGES.len(), 4);
    }

    #[test]
    fn test_localized_subject() {
        assert!(localized_subject("en", 1).contains("1 new event indexed"));
        assert!(localized_subject("en", 2).contains("2 new events indexed"));
        assert!(localized_subject("es", 3).contains("3"));
        assert!(localized_subject("zh", 4).contains("已索引 4"));
        assert!(localized_subject("ja", 5).contains("5"));
    }
}
