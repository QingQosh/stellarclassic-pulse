use axum::{routing::get, Router};
use sqlx::PgPool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::handlers;

pub fn create_router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/events", get(handlers::get_events))
        .route("/events/:contract_id", get(handlers::get_events_by_contract))
        .route("/events/tx/:tx_hash", get(handlers::get_events_by_tx))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(pool)
}
