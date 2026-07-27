/// Cross-chain event correlation and causality tracking
/// Issue #682: Implement cross-chain event correlation

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::collections::HashMap;

/// Represents a unique transaction identifier across chains
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionId {
    /// Chain network identifier (e.g., "soroban-mainnet", "soroban-testnet")
    pub chain: String,
    /// Transaction hash or identifier on the source chain
    pub tx_hash: String,
}

impl TransactionId {
    pub fn new(chain: impl Into<String>, tx_hash: impl Into<String>) -> Self {
        Self {
            chain: chain.into(),
            tx_hash: tx_hash.into(),
        }
    }
}

/// Causality relationship between events
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CausalityType {
    /// Event A directly caused Event B
    Direct,
    /// Event A indirectly caused Event B through intermediate events
    Indirect,
    /// Events are related through a common ancestor
    Related,
    /// Events are in the same transaction but no direct causality
    Sequential,
}

/// Represents a correlation between two events across potentially different chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCorrelation {
    /// Unique correlation ID
    pub id: String,
    /// First event in the correlation
    pub source_event_id: String,
    /// Second event in the correlation
    pub target_event_id: String,
    /// Chain of the source event
    pub source_chain: String,
    /// Chain of the target event
    pub target_chain: String,
    /// Type of causality relationship
    pub causality: CausalityType,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Reason for the correlation
    pub reason: String,
    /// Timestamp of correlation detection
    pub detected_at: DateTime<Utc>,
    /// Optional metadata for the correlation
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EventCorrelation {
    pub fn new(
        source_event_id: impl Into<String>,
        target_event_id: impl Into<String>,
        source_chain: impl Into<String>,
        target_chain: impl Into<String>,
        causality: CausalityType,
        confidence: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source_event_id: source_event_id.into(),
            target_event_id: target_event_id.into(),
            source_chain: source_chain.into(),
            target_chain: target_chain.into(),
            causality,
            confidence,
            reason: reason.into(),
            detected_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

/// Represents a complete trace of related events across chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainTrace {
    /// Unique trace ID
    pub id: String,
    /// Root transaction that started the cross-chain flow
    pub root_transaction: TransactionId,
    /// All events involved in this trace
    pub events: Vec<TraceEvent>,
    /// All correlations between events
    pub correlations: Vec<EventCorrelation>,
    /// Chain sequence showing how the trace flows across chains
    pub chain_sequence: Vec<String>,
    /// Timestamp of trace creation
    pub created_at: DateTime<Utc>,
    /// Total confidence score for the entire trace
    pub overall_confidence: f64,
}

/// Represents a single event within a cross-chain trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub event_id: String,
    pub chain: String,
    pub contract_id: String,
    pub event_type: String,
    pub tx_hash: String,
    pub ledger: u64,
    pub ledger_close_time: DateTime<Utc>,
    /// The event's position in the causal chain
    pub depth: u32,
    /// Confidence that this event is part of the trace
    pub confidence: f64,
}

/// Builder for constructing cross-chain traces
pub struct CrossChainTraceBuilder {
    root_transaction: Option<TransactionId>,
    events: Vec<TraceEvent>,
    correlations: Vec<EventCorrelation>,
    chain_sequence: Vec<String>,
}

impl CrossChainTraceBuilder {
    pub fn new(root_tx: TransactionId) -> Self {
        Self {
            root_transaction: Some(root_tx),
            events: Vec::new(),
            correlations: Vec::new(),
            chain_sequence: Vec::new(),
        }
    }

    pub fn add_event(mut self, event: TraceEvent) -> Self {
        if !self.chain_sequence.contains(&event.chain) {
            self.chain_sequence.push(event.chain.clone());
        }
        self.events.push(event);
        self
    }

    pub fn add_correlation(mut self, correlation: EventCorrelation) -> Self {
        self.correlations.push(correlation);
        self
    }

    pub fn build(self) -> Option<CrossChainTrace> {
        let root_tx = self.root_transaction?;
        let overall_confidence = if self.correlations.is_empty() {
            1.0
        } else {
            let sum: f64 = self.correlations.iter().map(|c| c.confidence).sum();
            sum / self.correlations.len() as f64
        };

        Some(CrossChainTrace {
            id: Uuid::new_v4().to_string(),
            root_transaction: root_tx,
            events: self.events,
            correlations: self.correlations,
            chain_sequence: self.chain_sequence,
            created_at: Utc::now(),
            overall_confidence,
        })
    }
}

/// Correlation engine for detecting cross-chain relationships
pub struct CorrelationEngine {
    /// Similarity threshold for matching events (0.0 to 1.0)
    pub similarity_threshold: f64,
    /// Maximum time window for correlation (in seconds)
    pub time_window_secs: u64,
}

impl CorrelationEngine {
    pub fn new() -> Self {
        Self {
            similarity_threshold: 0.75,
            time_window_secs: 300, // 5 minutes
        }
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.similarity_threshold = threshold;
        self
    }

    pub fn with_time_window(mut self, secs: u64) -> Self {
        self.time_window_secs = secs;
        self
    }

    /// Calculate similarity between two events
    /// Returns a score between 0.0 and 1.0
    pub fn calculate_similarity(
        &self,
        event1: &TraceEvent,
        event2: &TraceEvent,
    ) -> f64 {
        let mut score = 0.0;
        let mut factors = 0;

        // Same event type (strong indicator)
        if event1.event_type == event2.event_type {
            score += 0.4;
        }
        factors += 1;

        // Same contract ID (moderate indicator)
        if event1.contract_id == event2.contract_id {
            score += 0.3;
        }
        factors += 1;

        // Related ledgers (weak indicator)
        let ledger_diff = (event1.ledger as i64 - event2.ledger as i64).abs();
        if ledger_diff <= 10 {
            score += 0.3;
        } else if ledger_diff <= 100 {
            score += 0.15;
        }
        factors += 1;

        if factors > 0 {
            score / factors as f64
        } else {
            0.0
        }
    }

    /// Detect causality between events
    pub fn detect_causality(
        &self,
        source: &TraceEvent,
        target: &TraceEvent,
    ) -> Option<CausalityType> {
        // Same chain, sequential execution
        if source.chain == target.chain && source.depth < target.depth {
            return Some(CausalityType::Sequential);
        }

        // Different chains, potential cross-chain causality
        if source.chain != target.chain {
            let similarity = self.calculate_similarity(source, target);
            if similarity >= self.similarity_threshold {
                return Some(CausalityType::Direct);
            }
        }

        None
    }
}

impl Default for CorrelationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_id_creation() {
        let tx_id = TransactionId::new("soroban-mainnet", "abc123");
        assert_eq!(tx_id.chain, "soroban-mainnet");
        assert_eq!(tx_id.tx_hash, "abc123");
    }

    #[test]
    fn test_event_correlation_creation() {
        let correlation = EventCorrelation::new(
            "event1",
            "event2",
            "chain1",
            "chain2",
            CausalityType::Direct,
            0.95,
            "Direct causality detected",
        );

        assert_eq!(correlation.source_event_id, "event1");
        assert_eq!(correlation.target_event_id, "event2");
        assert_eq!(correlation.causality, CausalityType::Direct);
        assert_eq!(correlation.confidence, 0.95);
    }

    #[test]
    fn test_correlation_engine_similarity() {
        let engine = CorrelationEngine::new();

        let event1 = TraceEvent {
            event_id: "e1".to_string(),
            chain: "chain1".to_string(),
            contract_id: "contract1".to_string(),
            event_type: "invoke".to_string(),
            tx_hash: "tx1".to_string(),
            ledger: 1000,
            ledger_close_time: Utc::now(),
            depth: 0,
            confidence: 1.0,
        };

        let event2 = TraceEvent {
            event_id: "e2".to_string(),
            chain: "chain2".to_string(),
            contract_id: "contract1".to_string(),
            event_type: "invoke".to_string(),
            tx_hash: "tx2".to_string(),
            ledger: 1005,
            ledger_close_time: Utc::now(),
            depth: 1,
            confidence: 1.0,
        };

        let similarity = engine.calculate_similarity(&event1, &event2);
        assert!(similarity > 0.5);
    }

    #[test]
    fn test_cross_chain_trace_builder() {
        let root_tx = TransactionId::new("soroban-mainnet", "root_tx");
        let builder = CrossChainTraceBuilder::new(root_tx.clone());

        let event = TraceEvent {
            event_id: "e1".to_string(),
            chain: "soroban-mainnet".to_string(),
            contract_id: "contract1".to_string(),
            event_type: "invoke".to_string(),
            tx_hash: "tx1".to_string(),
            ledger: 1000,
            ledger_close_time: Utc::now(),
            depth: 0,
            confidence: 1.0,
        };

        let trace = builder
            .add_event(event)
            .build();

        assert!(trace.is_some());
        let trace = trace.unwrap();
        assert_eq!(trace.root_transaction.chain, "soroban-mainnet");
        assert_eq!(trace.events.len(), 1);
    }

    #[test]
    fn test_causality_type_detection() {
        let engine = CorrelationEngine::with_threshold(0.5);

        let source = TraceEvent {
            event_id: "e1".to_string(),
            chain: "chain1".to_string(),
            contract_id: "contract1".to_string(),
            event_type: "invoke".to_string(),
            tx_hash: "tx1".to_string(),
            ledger: 1000,
            ledger_close_time: Utc::now(),
            depth: 0,
            confidence: 1.0,
        };

        let target = TraceEvent {
            event_id: "e2".to_string(),
            chain: "chain2".to_string(),
            contract_id: "contract1".to_string(),
            event_type: "invoke".to_string(),
            tx_hash: "tx2".to_string(),
            ledger: 1005,
            ledger_close_time: Utc::now(),
            depth: 1,
            confidence: 1.0,
        };

        let causality = engine.detect_causality(&source, &target);
        assert!(causality.is_some());
    }
}
