use axum::{extract::{Path, Query, State}, Json};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{error::AppError, models::{Event, PaginationParams}};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn get_events(
    State(pool): State<PgPool>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, AppError> {
    let limit = params.limit();
    let offset = params.offset();

    let events: Vec<Event> = sqlx::query_as(
        "SELECT * FROM events ORDER BY ledger DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await?;

    Ok(Json(json!({
        "data": events,
        "total": total,
        "page": params.page.unwrap_or(1),
        "limit": limit
    })))
}

pub async fn get_events_by_contract(
    State(pool): State<PgPool>,
    Path(contract_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, AppError> {
    let limit = params.limit();
    let offset = params.offset();

    let events: Vec<Event> = sqlx::query_as(
        "SELECT * FROM events WHERE contract_id = $1 ORDER BY ledger DESC LIMIT $2 OFFSET $3",
    )
    .bind(&contract_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    if events.is_empty() {
        return Err(AppError::NotFound);
    }

    Ok(Json(json!({ "data": events, "contract_id": contract_id })))
}

pub async fn get_events_by_tx(
    State(pool): State<PgPool>,
    Path(tx_hash): Path<String>,
) -> Result<Json<Value>, AppError> {
    let events: Vec<Event> = sqlx::query_as(
        "SELECT * FROM events WHERE tx_hash = $1 ORDER BY ledger DESC",
    )
    .bind(&tx_hash)
    .fetch_all(&pool)
    .await?;

    if events.is_empty() {
        return Err(AppError::NotFound);
    }

    Ok(Json(json!({ "data": events, "tx_hash": tx_hash })))
}
