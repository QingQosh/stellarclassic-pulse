//! End-to-end test suite — issue #654
//!
//! These tests target a **live** SorobanPulse stack started by
//! `docker compose -f docker-compose.e2e.yml up --build --wait`.
//!
//! They are intentionally separated from the unit/integration suite so they
//! can be gated behind the `E2E_BASE_URL` environment variable:
//!
//! ```bash
//! # Spin up the stack
//! docker compose -f docker-compose.e2e.yml up --build --wait
//!
//! # Run E2E tests
//! E2E_BASE_URL=http://localhost:3001 \
//! E2E_WEBHOOK_URL=http://localhost:9001 \
//! E2E_RPC_ADMIN_URL=http://localhost:8080 \
//! cargo test --test e2e_tests -- --test-threads=1
//!
//! # Tear down
//! docker compose -f docker-compose.e2e.yml down -v
//! ```
//!
//! When `E2E_BASE_URL` is not set the whole suite is skipped so that
//! `cargo test` in a plain CI job (without Docker) does not fail.

use serde_json::Value;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base_url() -> Option<String> {
    std::env::var("E2E_BASE_URL").ok()
}

fn webhook_admin_url() -> String {
    std::env::var("E2E_WEBHOOK_URL").unwrap_or_else(|_| "http://localhost:9001".into())
}

fn rpc_admin_url() -> String {
    std::env::var("E2E_RPC_ADMIN_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Poll `f` until it returns `true` or the timeout expires.
async fn wait_until<F, Fut>(f: F, timeout: Duration, interval: Duration) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if f().await {
            return true;
        }
        tokio::time::sleep(interval).await;
    }
    false
}

/// GET `url` and deserialise the body as JSON.
async fn get_json(url: &str) -> reqwest::Result<Value> {
    reqwest::get(url).await?.json::<Value>().await
}

/// POST JSON to `url` and return the response.
async fn post_json(url: &str, body: &Value) -> reqwest::Result<reqwest::Response> {
    reqwest::Client::new().post(url).json(body).send().await
}

/// Inject a WireMock mapping to make the RPC return a set of events.
async fn stub_rpc_events(rpc_admin: &str, events: Vec<Value>, latest_ledger: u64) {
    let mapping = serde_json::json!({
        "name": "getEvents-with-data",
        "priority": 1,
        "request": {
            "method": "POST",
            "url": "/",
            "bodyPatterns": [{ "contains": "\"getEvents\"" }]
        },
        "response": {
            "status": 200,
            "headers": { "Content-Type": "application/json" },
            "jsonBody": {
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "events": events,
                    "latestLedger": latest_ledger
                }
            }
        }
    });
    reqwest::Client::new()
        .post(format!("{rpc_admin}/__admin/mappings"))
        .json(&mapping)
        .send()
        .await
        .expect("failed to inject WireMock mapping");
}

/// Remove all non-default WireMock stubs (resets to base mappings file).
async fn reset_rpc_stubs(rpc_admin: &str) {
    reqwest::Client::new()
        .post(format!("{rpc_admin}/__admin/reset"))
        .send()
        .await
        .expect("failed to reset WireMock stubs");
}

/// Clear the webhook receiver's recorded deliveries.
async fn clear_webhook_deliveries(webhook_admin: &str) {
    reqwest::Client::new()
        .delete(format!("{webhook_admin}/received"))
        .send()
        .await
        .expect("failed to clear webhook deliveries");
}

// ---------------------------------------------------------------------------
// Test: health check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_health_check_returns_ok() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_health_check_returns_ok");
        return;
    };

    let body = get_json(&format!("{base}/healthz/ready"))
        .await
        .expect("GET /healthz/ready failed");

    assert_eq!(body["status"], "ok", "health status should be ok: {body}");
    assert_eq!(body["db"], "ok", "db should be ok: {body}");
    assert_eq!(body["indexer"], "ok", "indexer should be ok: {body}");
}

// ---------------------------------------------------------------------------
// Test: event indexing flow
// ---------------------------------------------------------------------------

/// Verify that when the RPC stub returns a new event the indexer picks it up
/// and it becomes visible via the REST API within a reasonable timeout.
#[tokio::test]
async fn e2e_event_indexing_flow() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_event_indexing_flow");
        return;
    };
    let rpc_admin = rpc_admin_url();

    // Inject one event into the RPC stub.
    let contract_id = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";
    let tx_hash = "a".repeat(64);

    stub_rpc_events(
        &rpc_admin,
        vec![serde_json::json!({
            "type": "contract",
            "id": "0000000004294967296-0000000000",
            "contractId": contract_id,
            "txHash": tx_hash,
            "ledger": 1001,
            "ledgerClosedAt": "2026-03-14T00:01:00Z",
            "pagingToken": "0000000004294967296-0000000000",
            "inSuccessfulContractCall": true,
            "value": { "xdr": "AAAAAQ==" },
            "topic": [{ "xdr": "AAAAAQ==" }]
        })],
        1001,
    )
    .await;

    // Poll until the event appears in the API (up to 30 s — one full poll cycle).
    let appeared = wait_until(
        || {
            let url = format!("{base}/v1/events/{contract_id}");
            async move {
                match get_json(&url).await {
                    Ok(v) => v["data"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
                    Err(_) => false,
                }
            }
        },
        Duration::from_secs(30),
        Duration::from_secs(2),
    )
    .await;

    // Restore default stub so other tests are not affected.
    reset_rpc_stubs(&rpc_admin).await;

    assert!(appeared, "event should appear in API within 30 s after indexing");

    // Verify the event fields.
    let body = get_json(&format!("{base}/v1/events/{contract_id}"))
        .await
        .expect("GET /v1/events/{contract_id} failed");
    let events = body["data"].as_array().expect("data should be an array");
    assert!(!events.is_empty(), "should have at least one event");
    let ev = &events[0];
    assert_eq!(ev["contract_id"], contract_id);
    assert_eq!(ev["tx_hash"], tx_hash);
    assert_eq!(ev["ledger"], 1001);
    assert_eq!(ev["event_type"], "contract");
}

// ---------------------------------------------------------------------------
// Test: pagination
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_pagination_returns_correct_pages() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_pagination_returns_correct_pages");
        return;
    };

    // Seed via direct SQL is handled by the `seed.sql` script run before the
    // test suite; we rely on Contract A having 50 events.
    let page1 = get_json(&format!("{base}/v1/events?page=1&limit=10"))
        .await
        .expect("page 1 request failed");
    let page2 = get_json(&format!("{base}/v1/events?page=2&limit=10"))
        .await
        .expect("page 2 request failed");

    let p1_data = page1["data"].as_array().expect("data should be array");
    let p2_data = page2["data"].as_array().expect("data should be array");

    assert_eq!(p1_data.len(), 10, "page 1 should have 10 events");
    assert_eq!(p2_data.len(), 10, "page 2 should have 10 events");

    // IDs on page 1 and page 2 must not overlap.
    let p1_ids: std::collections::HashSet<&str> =
        p1_data.iter().filter_map(|e| e["id"].as_str()).collect();
    let p2_ids: std::collections::HashSet<&str> =
        p2_data.iter().filter_map(|e| e["id"].as_str()).collect();
    assert!(
        p1_ids.is_disjoint(&p2_ids),
        "pages should not contain duplicate events"
    );
}

// ---------------------------------------------------------------------------
// Test: ledger range filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_ledger_range_filter() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_ledger_range_filter");
        return;
    };

    let body = get_json(&format!("{base}/v1/events?from_ledger=1001&to_ledger=1005"))
        .await
        .expect("ledger range request failed");

    let events = body["data"].as_array().expect("data should be array");
    for ev in events {
        let ledger = ev["ledger"].as_u64().expect("ledger should be u64");
        assert!(
            (1001..=1005).contains(&ledger),
            "event ledger {ledger} outside requested range"
        );
    }
}

// ---------------------------------------------------------------------------
// Test: event type filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_event_type_filter_contract() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_event_type_filter_contract");
        return;
    };

    let body = get_json(&format!("{base}/v1/events?event_type=contract"))
        .await
        .expect("event_type filter request failed");

    let events = body["data"].as_array().expect("data should be array");
    for ev in events {
        assert_eq!(
            ev["event_type"], "contract",
            "filtered results should only contain contract events"
        );
    }
}

#[tokio::test]
async fn e2e_invalid_event_type_returns_400() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_invalid_event_type_returns_400");
        return;
    };

    let resp = reqwest::get(&format!("{base}/v1/events?event_type=unknown_type"))
        .await
        .expect("request failed");

    assert_eq!(
        resp.status(),
        400,
        "unknown event_type should return 400 Bad Request"
    );
}

// ---------------------------------------------------------------------------
// Test: GET by transaction hash
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_get_events_by_tx_hash() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_get_events_by_tx_hash");
        return;
    };

    // Ledger 1 of the seed data has tx_hash "0...01" (padded to 64 chars).
    let tx_hash = "0".repeat(63) + "1";
    let body = get_json(&format!("{base}/v1/events/tx/{tx_hash}"))
        .await
        .expect("GET /v1/events/tx/{hash} failed");

    let events = body["data"].as_array().expect("data should be array");
    // The endpoint returns an empty array for unknown hashes — that's fine.
    // If data is non-empty every event must carry that tx_hash.
    for ev in events {
        assert_eq!(ev["tx_hash"], tx_hash);
    }
}

// ---------------------------------------------------------------------------
// Test: SSE stream connects and receives keep-alive pings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_sse_stream_connects_and_pings() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_sse_stream_connects_and_pings");
        return;
    };

    // Open an SSE connection with a short timeout.  We just verify we receive
    // the correct Content-Type and at least one `ping` event within 10 s.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let resp = client
        .get(&format!("{base}/v1/events/stream"))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("SSE connection failed");

    assert_eq!(resp.status(), 200, "SSE endpoint should return 200");
    assert!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|ct| ct.contains("text/event-stream"))
            .unwrap_or(false),
        "SSE response must have Content-Type: text/event-stream"
    );

    // Read bytes for up to 10 s.  The server emits a `ping` every 5 s (set in
    // docker-compose.e2e.yml via SSE_KEEPALIVE_SECS=5).
    let bytes = resp.bytes().await.unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("event: ping"),
        "SSE stream should emit a ping event within 10 s"
    );
}

// ---------------------------------------------------------------------------
// Test: subscription delivery flow
// ---------------------------------------------------------------------------

/// Creates a subscription, then injects an event via the RPC stub and verifies
/// that the subscription mechanism records the notification.
///
/// Note: SorobanPulse delivers subscriptions in-process (not via an external
/// queue in this config), so we validate that the indexed event is visible via
/// the REST API and that subscription metadata is returned correctly.
#[tokio::test]
async fn e2e_subscription_creation_and_listing() {
    let Some(base) = base_url() else {
        eprintln!(
            "E2E_BASE_URL not set — skipping e2e_subscription_creation_and_listing"
        );
        return;
    };

    let contract_id = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";

    // Create a webhook subscription.
    let webhook_url = format!("{}/webhook", webhook_admin_url());
    let body = serde_json::json!({
        "contract_id": contract_id,
        "webhook_url": webhook_url,
        "event_types": ["contract"]
    });

    let resp = post_json(&format!("{base}/v1/subscriptions"), &body)
        .await
        .expect("POST /v1/subscriptions failed");

    let status = resp.status();
    assert!(
        status == 200 || status == 201,
        "subscription creation should succeed (got {status})"
    );

    let created: Value = resp.json().await.expect("response body should be JSON");
    assert!(
        created["id"].is_string() || created["subscription_id"].is_string(),
        "response should include an id field"
    );

    // List subscriptions and verify ours is present.
    let list = get_json(&format!("{base}/v1/subscriptions"))
        .await
        .expect("GET /v1/subscriptions failed");
    let subs = list["data"]
        .as_array()
        .or_else(|| list.as_array())
        .expect("subscriptions response should be an array");

    assert!(
        !subs.is_empty(),
        "subscriptions list should contain at least one entry"
    );
}

// ---------------------------------------------------------------------------
// Test: webhook delivery flow
// ---------------------------------------------------------------------------

/// Verifies the full webhook delivery pipeline:
/// 1. Register a webhook subscription pointing at the local webhook receiver.
/// 2. Inject an event via the RPC stub.
/// 3. Wait for the indexer to pick up the event.
/// 4. Assert that the webhook receiver recorded at least one delivery.
#[tokio::test]
async fn e2e_webhook_delivery_flow() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_webhook_delivery_flow");
        return;
    };
    let rpc_admin = rpc_admin_url();
    let webhook_admin = webhook_admin_url();

    // Clear any previous webhook deliveries.
    clear_webhook_deliveries(&webhook_admin).await;

    let contract_id = "CDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDFCT4";
    let tx_hash = "d".repeat(64);

    // Register webhook subscription.
    let webhook_url = format!("{webhook_admin}/webhook");
    let sub_body = serde_json::json!({
        "contract_id": contract_id,
        "webhook_url": webhook_url,
        "event_types": ["contract"]
    });
    let sub_resp = post_json(&format!("{base}/v1/subscriptions"), &sub_body)
        .await
        .expect("failed to register subscription");
    assert!(
        sub_resp.status().is_success(),
        "subscription registration failed with status {}",
        sub_resp.status()
    );

    // Inject a matching event via the RPC stub.
    stub_rpc_events(
        &rpc_admin,
        vec![serde_json::json!({
            "type": "contract",
            "id": "0000000008589934592-0000000000",
            "contractId": contract_id,
            "txHash": tx_hash,
            "ledger": 2001,
            "ledgerClosedAt": "2026-03-14T01:00:00Z",
            "pagingToken": "0000000008589934592-0000000000",
            "inSuccessfulContractCall": true,
            "value": { "xdr": "AAAAAQ==" },
            "topic": [{ "xdr": "AAAAAQ==" }]
        })],
        2001,
    )
    .await;

    // Wait for webhook delivery (up to 30 s).
    let delivered = wait_until(
        || {
            let url = format!("{webhook_admin}/received");
            async move {
                match get_json(&url).await {
                    Ok(v) => v.as_array().map(|a| !a.is_empty()).unwrap_or(false),
                    Err(_) => false,
                }
            }
        },
        Duration::from_secs(30),
        Duration::from_secs(2),
    )
    .await;

    reset_rpc_stubs(&rpc_admin).await;
    clear_webhook_deliveries(&webhook_admin).await;

    assert!(
        delivered,
        "webhook receiver should have received at least one delivery within 30 s"
    );
}

// ---------------------------------------------------------------------------
// Test: metrics endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_metrics_endpoint_returns_prometheus_format() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_metrics_endpoint_returns_prometheus_format");
        return;
    };

    let resp = reqwest::get(&format!("{base}/metrics"))
        .await
        .expect("GET /metrics failed");

    assert_eq!(resp.status(), 200, "/metrics should return 200");
    let body = resp.text().await.expect("failed to read metrics body");
    assert!(
        body.contains("soroban_pulse_events_indexed_total"),
        "metrics body should contain soroban_pulse_events_indexed_total"
    );
    assert!(
        body.contains("soroban_pulse_indexer_current_ledger"),
        "metrics body should contain soroban_pulse_indexer_current_ledger"
    );
}

// ---------------------------------------------------------------------------
// Test: rate limiting
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_rate_limiting_is_disabled_in_e2e_env() {
    let Some(base) = base_url() else {
        eprintln!("E2E_BASE_URL not set — skipping e2e_rate_limiting_is_disabled_in_e2e_env");
        return;
    };

    // The E2E compose sets RATE_LIMIT_PER_MINUTE=0 (unlimited).
    // Fire 20 rapid requests and assert none returns 429.
    let client = reqwest::Client::new();
    for _ in 0..20 {
        let resp = client
            .get(&format!("{base}/v1/events"))
            .send()
            .await
            .expect("request failed");
        assert_ne!(
            resp.status(),
            429,
            "rate limiting should be disabled in E2E env"
        );
    }
}

// ---------------------------------------------------------------------------
// Test: deprecated unversioned routes return Deprecation header
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_deprecated_routes_return_deprecation_header() {
    let Some(base) = base_url() else {
        eprintln!(
            "E2E_BASE_URL not set — skipping e2e_deprecated_routes_return_deprecation_header"
        );
        return;
    };

    let resp = reqwest::get(&format!("{base}/events"))
        .await
        .expect("GET /events failed");

    assert_eq!(resp.status(), 200, "/events should return 200");
    assert!(
        resp.headers().contains_key("deprecation"),
        "deprecated route should include Deprecation header"
    );
}
