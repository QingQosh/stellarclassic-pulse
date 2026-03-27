use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

// The local module is also named `metrics`, which shadows the external crate
// of the same name. Use an explicit extern-crate alias to disambiguate.
extern crate metrics as m;

/// Initialize the Prometheus metrics exporter
pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus exporter")
}

/// Record events indexed
pub fn record_events_indexed(count: u64) {
    m::counter!("soroban_pulse_events_indexed_total", count);
}

/// Update the current ledger being processed
pub fn update_current_ledger(ledger: u64) {
    m::gauge!("soroban_pulse_indexer_current_ledger", ledger as f64);
}

/// Update the latest ledger from RPC
pub fn update_latest_ledger(ledger: u64) {
    m::gauge!("soroban_pulse_indexer_latest_ledger", ledger as f64);
}

/// Update the indexer lag
pub fn update_indexer_lag(lag: u64) {
    m::gauge!("soroban_pulse_indexer_lag_ledgers", lag as f64);
}

/// Record an RPC error
pub fn record_rpc_error() {
    m::counter!("soroban_pulse_rpc_errors_total", 1u64);
}

/// Record HTTP request duration
pub fn record_http_request_duration(duration: std::time::Duration, method: &str, route: &str, status: &str) {
    m::histogram!("soroban_pulse_http_request_duration_seconds", duration.as_secs_f64(), "method" => method.to_string(), "route" => route.to_string(), "status" => status.to_string());
}
