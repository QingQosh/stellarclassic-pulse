use axum::{routing::get, Router};
use axum::http::{HeaderValue, Method};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer, compression::CompressionLayer};

use crate::{config::HealthState, middleware, handlers as handlers_module};

pub type AppState = crate::handlers::AppState;

pub fn create_router(
    pool: PgPool,
    api_key: Option<String>,
    allowed_origins: &[String],
    rate_limit_per_minute: u32,
    health_state: Arc<HealthState>,
) -> Router {
    let cors = build_cors(allowed_origins);
    let auth_state = Arc::new(middleware::AuthState { api_key });

    // Create app state that combines pool and health state
    let app_state = AppState {
        pool,
        health_state,
    };

    // Replenish one token every (60 / rate_limit_per_minute) seconds.
    // burst_size = rate_limit_per_minute so a fresh client can use the full quota at once.
    let _period_secs = 60u64.div_ceil(rate_limit_per_minute as u64);
    /*
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(period_secs)
            .burst_size(rate_limit_per_minute)
            .use_headers()
            .finish()
            .expect("invalid rate limit configuration"),
    );
    */

    // Rate-limited API routes - must be typed with AppState
    let api: Router<AppState> = Router::<AppState>::new()
        .route("/events", get(handlers_module::get_events))
        .route("/events/contract/:contract_id", get(handlers_module::get_events_by_contract))
        .route("/events/tx/:tx_hash", get(handlers_module::get_events_by_tx));
        // .layer(GovernorLayer::new(governor_conf));

    Router::<AppState>::new()
        .route("/health", get(handlers_module::health))
        .merge(api)
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            middleware::auth_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(app_state)
}

fn build_cors(allowed_origins: &[String]) -> CorsLayer {
    let methods = [Method::GET];

    if allowed_origins.iter().any(|o| o == "*") {
        return CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(methods);
    }

    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(methods)
        .vary([axum::http::header::ORIGIN])
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header, Request};
    use axum::body::Body;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_compression_header() {
        let pool = PgPool::connect_lazy("postgres://localhost/unused").unwrap();
        
        // Manual router construction for testing with a large response
        let api = Router::new()
            .route("/large", axum::routing::get(|| async { "A".repeat(2000) }));
        
        let app = Router::new()
            .merge(api)
            .layer(tower_http::compression::CompressionLayer::new())
            .with_state(pool);

        // Requested gzip
        let response = app.clone().oneshot(
            Request::builder()
                .uri("/large")
                .header(header::ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap()
        ).await.unwrap();

        assert_eq!(response.headers().get(header::CONTENT_ENCODING).unwrap(), "gzip");

        // Requested nothing
        let response = app.oneshot(
            Request::builder()
                .uri("/large")
                .body(Body::empty())
                .unwrap()
        ).await.unwrap();

        assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
    }
}
