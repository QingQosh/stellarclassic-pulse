use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use soroban_pulse::config::{HealthState, IndexerState};
use soroban_pulse::metrics::init_metrics;
use soroban_pulse::routes::create_router;

fn make_router(pool: PgPool) -> axum::Router {
    let health_state = Arc::new(HealthState::new(60));
    health_state.update_last_poll();
    let indexer_state = Arc::new(IndexerState::new());
    let prometheus_handle = init_metrics();
    let api_keys = [].into_iter().collect();
    let config = soroban_pulse::config::Config::default();
    create_router(
        pool,
        api_keys,
        &[],
        60,
        health_state,
        indexer_state,
        prometheus_handle,
        15000,
        config,
    )
}

async fn insert_event_with_ts(
    pool: &PgPool,
    contract_id: &str,
    ledger: i64,
    timestamp: DateTime<Utc>,
    created_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    if let Some(ca) = created_at {
        sqlx::query(
            "INSERT INTO events (contract_id, event_type, tx_hash, ledger, timestamp, event_data, created_at) \
             VALUES ($1, 'contract', $2, $3, $4, $5, $6)",
        )
        .bind(contract_id)
        .bind(format!("{:064x}", rand::random::<u64>()))
        .bind(ledger)
        .bind(timestamp)
        .bind(json!({}))
        .bind(ca)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO events (contract_id, event_type, tx_hash, ledger, timestamp, event_data) \
             VALUES ($1, 'contract', $2, $3, $4, $5)",
        )
        .bind(contract_id)
        .bind(format!("{:064x}", rand::random::<u64>()))
        .bind(ledger)
        .bind(timestamp)
        .bind(json!({}))
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn get_events_sort_by_timestamp_desc(pool: PgPool) -> sqlx::Result<()> {
    let base = Utc::now();
    let t1 = base - Duration::hours(3);
    let t2 = base - Duration::hours(2);
    let t3 = base - Duration::hours(1);

    insert_event_with_ts(
        &pool,
        "C000000000000000000000000000000000000000000000000000000",
        100,
        t1,
        None,
    )
    .await?;
    insert_event_with_ts(
        &pool,
        "C111111111111111111111111111111111111111111111111111111",
        200,
        t2,
        None,
    )
    .await?;
    insert_event_with_ts(
        &pool,
        "C222222222222222222222222222222222222222222222222222222",
        300,
        t3,
        None,
    )
    .await?;

    let app = make_router(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/events?sort_by=timestamp&sort=desc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
    // Expect t3, t2, t1
    let got0 = data[0]["timestamp"]
        .as_str()
        .unwrap()
        .parse::<DateTime<Utc>>()
        .unwrap();
    let got1 = data[1]["timestamp"]
        .as_str()
        .unwrap()
        .parse::<DateTime<Utc>>()
        .unwrap();
    let got2 = data[2]["timestamp"]
        .as_str()
        .unwrap()
        .parse::<DateTime<Utc>>()
        .unwrap();
    assert!(got0 >= got1 && got1 >= got2);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn get_events_sort_by_created_at_asc(pool: PgPool) -> sqlx::Result<()> {
    let base = Utc::now();
    let c1 = base - Duration::hours(3);
    let c2 = base - Duration::hours(2);
    let c3 = base - Duration::hours(1);

    // Same timestamps but different created_at to ensure sorting by created_at works
    let ts = base - Duration::hours(6);
    insert_event_with_ts(
        &pool,
        "C010000000000000000000000000000000000000000000000000000",
        400,
        ts,
        Some(c1),
    )
    .await?;
    insert_event_with_ts(
        &pool,
        "C010000000000000000000000000000000000000000000000000001",
        401,
        ts,
        Some(c2),
    )
    .await?;
    insert_event_with_ts(
        &pool,
        "C010000000000000000000000000000000000000000000000000002",
        402,
        ts,
        Some(c3),
    )
    .await?;

    let app = make_router(pool.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/events?sort_by=created_at&sort=asc")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
    let got0 = data[0]["created_at"]
        .as_str()
        .unwrap()
        .parse::<DateTime<Utc>>()
        .unwrap();
    let got1 = data[1]["created_at"]
        .as_str()
        .unwrap()
        .parse::<DateTime<Utc>>()
        .unwrap();
    let got2 = data[2]["created_at"]
        .as_str()
        .unwrap()
        .parse::<DateTime<Utc>>()
        .unwrap();
    assert!(got0 <= got1 && got1 <= got2);

    Ok(())
}
