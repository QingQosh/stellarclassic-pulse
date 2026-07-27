/// Handle Soroban RPC cursor expiry with exponential backoff and fallback logic
/// Issue #685: Improve handling when Soroban RPC cursor expires mid-pagination

use std::time::Duration;
use tracing::{warn, info, error};

/// Detects if an RPC error is a cursor expiry error
pub fn is_cursor_expiry_error(error_msg: &str) -> bool {
    let cursor_expiry_indicators = [
        "cursor",
        "expired",
        "invalid cursor",
        "cursor is no longer valid",
        "pagination cursor expired",
    ];

    let lower_msg = error_msg.to_lowercase();
    cursor_expiry_indicators.iter().any(|indicator| lower_msg.contains(indicator))
}

/// Manages exponential backoff for cursor expiry scenarios
#[derive(Debug, Clone)]
pub struct CursorExpiryBackoff {
    /// Initial backoff duration in milliseconds
    initial_backoff_ms: u64,
    /// Maximum backoff duration in milliseconds
    max_backoff_ms: u64,
    /// Current backoff duration
    current_backoff_ms: u64,
    /// Number of retries with backoff
    retry_count: u32,
    /// Maximum number of retries before falling back to ledger start
    max_retries: u32,
}

impl CursorExpiryBackoff {
    /// Create a new cursor expiry backoff handler
    pub fn new() -> Self {
        Self::with_config(100, 30000, 5)
    }

    /// Create with custom configuration
    /// - initial_backoff_ms: Starting backoff duration (default: 100ms)
    /// - max_backoff_ms: Maximum backoff duration (default: 30 seconds)
    /// - max_retries: Maximum retry attempts before fallback (default: 5)
    pub fn with_config(initial_backoff_ms: u64, max_backoff_ms: u64, max_retries: u32) -> Self {
        Self {
            initial_backoff_ms,
            max_backoff_ms,
            current_backoff_ms: initial_backoff_ms,
            retry_count: 0,
            max_retries,
        }
    }

    /// Get the next backoff duration and increment retry count
    pub fn next_backoff(&mut self) -> Duration {
        self.retry_count += 1;

        // Calculate exponential backoff: initial * 2^(retry_count - 1)
        let exponential = self.initial_backoff_ms * (2_u64.pow(self.retry_count.saturating_sub(1)));
        self.current_backoff_ms = std::cmp::min(exponential, self.max_backoff_ms);

        info!(
            retry_count = self.retry_count,
            backoff_ms = self.current_backoff_ms,
            "Cursor expiry: applying exponential backoff"
        );

        Duration::from_millis(self.current_backoff_ms)
    }

    /// Check if we should fall back to fetching from ledger start
    pub fn should_fallback(&self) -> bool {
        self.retry_count >= self.max_retries
    }

    /// Get the current retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Reset backoff state
    pub fn reset(&mut self) {
        self.retry_count = 0;
        self.current_backoff_ms = self.initial_backoff_ms;
    }
}

impl Default for CursorExpiryBackoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cursor_expiry_error() {
        assert!(is_cursor_expiry_error("cursor is no longer valid"));
        assert!(is_cursor_expiry_error("Pagination cursor expired"));
        assert!(is_cursor_expiry_error("invalid cursor"));
        assert!(!is_cursor_expiry_error("network timeout"));
        assert!(!is_cursor_expiry_error("connection refused"));
    }

    #[test]
    fn test_cursor_expiry_backoff_exponential() {
        let mut backoff = CursorExpiryBackoff::with_config(100, 30000, 5);

        let b1 = backoff.next_backoff();
        assert_eq!(b1.as_millis(), 100); // 100 * 2^0

        let b2 = backoff.next_backoff();
        assert_eq!(b2.as_millis(), 200); // 100 * 2^1

        let b3 = backoff.next_backoff();
        assert_eq!(b3.as_millis(), 400); // 100 * 2^2

        let b4 = backoff.next_backoff();
        assert_eq!(b4.as_millis(), 800); // 100 * 2^3
    }

    #[test]
    fn test_cursor_expiry_backoff_max_cap() {
        let mut backoff = CursorExpiryBackoff::with_config(100, 1000, 10);

        // Keep incrementing until we hit max
        for _ in 0..10 {
            backoff.next_backoff();
        }

        // Should cap at max_backoff_ms
        assert!(backoff.current_backoff_ms <= 1000);
    }

    #[test]
    fn test_cursor_expiry_should_fallback() {
        let mut backoff = CursorExpiryBackoff::with_config(100, 30000, 3);

        assert!(!backoff.should_fallback());

        backoff.next_backoff();
        assert!(!backoff.should_fallback());

        backoff.next_backoff();
        assert!(!backoff.should_fallback());

        backoff.next_backoff();
        assert!(backoff.should_fallback());
    }

    #[test]
    fn test_cursor_expiry_backoff_reset() {
        let mut backoff = CursorExpiryBackoff::with_config(100, 30000, 5);

        backoff.next_backoff();
        backoff.next_backoff();
        assert_eq!(backoff.retry_count(), 2);

        backoff.reset();
        assert_eq!(backoff.retry_count(), 0);
        assert_eq!(backoff.current_backoff_ms, 100);
    }
}
