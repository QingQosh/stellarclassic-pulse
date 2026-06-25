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

use chrono::{DateTime, Timelike, Utc};

use crate::{metrics, models::SorobanEvent, retry_policy::RetryPolicy};

/// Issue #479: How often a notification channel flushes its batched events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schedule {
    /// Flush on every batch tick (legacy behavior — roughly once per minute).
    Immediate,
    /// One digest per hour, on the hour (UTC).
    HourlyDigest,
    /// One digest per day at the configured UTC hour (default 09:00).
    DailyDigest { hour: u32 },
    /// Flush according to a cron expression (UTC). Uses the `cron` crate's
    /// 6/7-field syntax (seconds first).
    CustomCron(String),
}

impl Schedule {
    /// Build a [`Schedule`] from the `EMAIL_SCHEDULE` value.
    ///
    /// Unknown values fall back to [`Schedule::Immediate`]. `daily_hour` is
    /// clamped to a valid 0–23 hour; `cron_expr` is only used for `custom_cron`.
    pub fn parse(value: &str, daily_hour: u32, cron_expr: Option<String>) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "hourly_digest" => Schedule::HourlyDigest,
            "daily_digest" => Schedule::DailyDigest {
                hour: daily_hour.min(23),
            },
            "custom_cron" => Schedule::CustomCron(cron_expr.unwrap_or_default()),
            _ => Schedule::Immediate,
        }
    }

    /// Whether a batch should be flushed at `now`, given the last successful
    /// send at `last_sent`. This is a pure function so it can be unit-tested
    /// without a running scheduler.
    pub fn is_due(&self, now: DateTime<Utc>, last_sent: DateTime<Utc>) -> bool {
        match self {
            Schedule::Immediate => true,
            Schedule::HourlyDigest => {
                // Due once the wall-clock hour advances past the last send.
                now.timestamp().div_euclid(3600) > last_sent.timestamp().div_euclid(3600)
            }
            Schedule::DailyDigest { hour } => {
                let scheduled = now
                    .date_naive()
                    .and_hms_opt(*hour, 0, 0)
                    .map(|naive| naive.and_utc());
                match scheduled {
                    Some(scheduled) => now >= scheduled && last_sent < scheduled,
                    None => false,
                }
            }
            Schedule::CustomCron(expr) => {
                use std::str::FromStr;
                match cron::Schedule::from_str(expr) {
                    Ok(schedule) => schedule
                        .after(&last_sent)
                        .next()
                        .is_some_and(|next| next <= now),
                    Err(_) => false,
                }
            }
        }
    }
}

/// Issue #479: A UTC quiet-hours window during which non-critical
/// notifications are suppressed (deferred until the window closes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHours {
    /// Minutes since UTC midnight when the window opens.
    start_min: u32,
    /// Minutes since UTC midnight when the window closes.
    end_min: u32,
}

impl QuietHours {
    /// Parse a `start`/`end` pair of `HH:MM` strings into a window.
    ///
    /// Returns `None` when either bound is missing or unparseable, or when the
    /// window is empty (start == end), which disables quiet hours.
    pub fn parse(start: Option<&str>, end: Option<&str>) -> Option<QuietHours> {
        let start_min = parse_hh_mm(start?)?;
        let end_min = parse_hh_mm(end?)?;
        if start_min == end_min {
            return None;
        }
        Some(QuietHours { start_min, end_min })
    }

    /// Whether `now` falls inside the quiet-hours window. Handles windows that
    /// wrap past midnight (e.g. 22:00–07:00).
    pub fn contains(&self, now: DateTime<Utc>) -> bool {
        let minute_of_day = now.hour() * 60 + now.minute();
        if self.start_min < self.end_min {
            minute_of_day >= self.start_min && minute_of_day < self.end_min
        } else {
            // Wrap-around window (e.g. 22:00–07:00).
            minute_of_day >= self.start_min || minute_of_day < self.end_min
        }
    }
}

/// Parse an `HH:MM` 24-hour string into minutes since midnight.
fn parse_hh_mm(value: &str) -> Option<u32> {
    let value = value.trim();
    let (h, m) = value.split_once(':')?;
    let hours: u32 = h.trim().parse().ok()?;
    let minutes: u32 = m.trim().parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(hours * 60 + minutes)
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
    /// Issue #479: when batched events are flushed.
    schedule: Schedule,
    /// Issue #479: optional UTC window during which delivery is suppressed.
    quiet_hours: Option<QuietHours>,
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
        schedule: Schedule,
        quiet_hours: Option<QuietHours>,
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
            schedule,
            quiet_hours,
            pool,
        }
    }

    /// Spawn a background task that batches events and sends emails every minute.
    pub fn spawn(
        self,
        mut event_rx: tokio::sync::broadcast::Receiver<SorobanEvent>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // Evaluate the schedule once a minute. Events accumulate in the
            // buffer until the configured schedule says a flush is due and we
            // are outside of any quiet-hours window (Issue #479).
            let mut batch_interval = interval(Duration::from_secs(60));
            batch_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let mut events_buffer: Vec<SorobanEvent> = Vec::new();
            let mut last_sent = Utc::now();

            loop {
                tokio::select! {
                    _ = batch_interval.tick() => {
                        let now = Utc::now();
                        if events_buffer.is_empty() || !self.schedule.is_due(now, last_sent) {
                            continue;
                        }
                        // Suppress (defer) non-critical notifications during
                        // quiet hours; the buffer is flushed once the window
                        // closes on a later tick.
                        if self.quiet_hours.is_some_and(|q| q.contains(now)) {
                            info!("In quiet hours, deferring email notification");
                            continue;
                        }
                        self.send_batch_email(&events_buffer).await;
                        events_buffer.clear();
                        last_sent = now;
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
                                // Channel closed, flush any remaining events and exit.
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
            ..Default::default()
        }
    }

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
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
            Schedule::Immediate,
            None,
            pool,
        );

        assert_eq!(notifier.smtp_host, "smtp.example.com");
        assert_eq!(notifier.smtp_port, 587);
        assert_eq!(notifier.from, "from@example.com");
        assert_eq!(notifier.to.len(), 1);
        assert_eq!(notifier.schedule, Schedule::Immediate);
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

    // --- Issue #479: schedule evaluation ---

    #[test]
    fn test_schedule_parse() {
        assert_eq!(Schedule::parse("immediate", 9, None), Schedule::Immediate);
        assert_eq!(
            Schedule::parse("hourly_digest", 9, None),
            Schedule::HourlyDigest
        );
        assert_eq!(
            Schedule::parse("daily_digest", 7, None),
            Schedule::DailyDigest { hour: 7 }
        );
        // Out-of-range hour is clamped.
        assert_eq!(
            Schedule::parse("daily_digest", 99, None),
            Schedule::DailyDigest { hour: 23 }
        );
        assert_eq!(
            Schedule::parse("custom_cron", 9, Some("0 0 * * * *".to_string())),
            Schedule::CustomCron("0 0 * * * *".to_string())
        );
        // Unknown values fall back to immediate.
        assert_eq!(Schedule::parse("weekly", 9, None), Schedule::Immediate);
    }

    #[test]
    fn test_immediate_always_due() {
        let now = ts("2026-06-25T03:00:00Z");
        assert!(Schedule::Immediate.is_due(now, now));
    }

    #[test]
    fn test_hourly_digest_due_on_new_hour() {
        let last = ts("2026-06-25T08:30:00Z");
        // Still the same hour -> not due.
        assert!(!Schedule::HourlyDigest.is_due(ts("2026-06-25T08:45:00Z"), last));
        // Crossed into a new hour -> due.
        assert!(Schedule::HourlyDigest.is_due(ts("2026-06-25T09:01:00Z"), last));
    }

    #[test]
    fn test_daily_digest_sends_once_per_day() {
        let schedule = Schedule::DailyDigest { hour: 9 };
        let last = ts("2026-06-24T09:00:00Z");

        // Before the scheduled hour today -> not due.
        assert!(!schedule.is_due(ts("2026-06-25T08:59:00Z"), last));
        // At/after the scheduled hour and not yet sent today -> due.
        assert!(schedule.is_due(ts("2026-06-25T09:00:00Z"), last));
        // Already sent today -> not due again.
        let sent_today = ts("2026-06-25T09:00:00Z");
        assert!(!schedule.is_due(ts("2026-06-25T18:00:00Z"), sent_today));
    }

    #[test]
    fn test_custom_cron_due() {
        // "At second 0 of minute 0 of every hour" (6-field cron, seconds first).
        let schedule = Schedule::CustomCron("0 0 * * * *".to_string());
        let last = ts("2026-06-25T08:30:00Z");
        // 09:00:00 occurs between last and now -> due.
        assert!(schedule.is_due(ts("2026-06-25T09:00:30Z"), last));
        // No top-of-hour boundary crossed yet -> not due.
        assert!(!schedule.is_due(ts("2026-06-25T08:45:00Z"), last));
    }

    #[test]
    fn test_custom_cron_invalid_expression_never_due() {
        let schedule = Schedule::CustomCron("not a cron".to_string());
        let last = ts("2026-06-25T08:30:00Z");
        assert!(!schedule.is_due(ts("2026-06-25T09:00:00Z"), last));
    }

    // --- Issue #479: quiet hours ---

    #[test]
    fn test_quiet_hours_parse() {
        assert!(QuietHours::parse(Some("22:00"), Some("07:00")).is_some());
        // Missing bound disables quiet hours.
        assert!(QuietHours::parse(Some("22:00"), None).is_none());
        // Empty window disables quiet hours.
        assert!(QuietHours::parse(Some("09:00"), Some("09:00")).is_none());
        // Invalid time disables quiet hours.
        assert!(QuietHours::parse(Some("25:00"), Some("07:00")).is_none());
    }

    #[test]
    fn test_quiet_hours_wraps_past_midnight() {
        let quiet = QuietHours::parse(Some("22:00"), Some("07:00")).unwrap();
        assert!(quiet.contains(ts("2026-06-25T23:30:00Z"))); // late night
        assert!(quiet.contains(ts("2026-06-25T03:00:00Z"))); // early morning
        assert!(!quiet.contains(ts("2026-06-25T12:00:00Z"))); // midday
        // Boundaries: start inclusive, end exclusive.
        assert!(quiet.contains(ts("2026-06-25T22:00:00Z")));
        assert!(!quiet.contains(ts("2026-06-25T07:00:00Z")));
    }

    #[test]
    fn test_quiet_hours_same_day_window() {
        let quiet = QuietHours::parse(Some("09:00"), Some("17:00")).unwrap();
        assert!(quiet.contains(ts("2026-06-25T12:00:00Z")));
        assert!(!quiet.contains(ts("2026-06-25T08:00:00Z")));
        assert!(!quiet.contains(ts("2026-06-25T18:00:00Z")));
    }
}
