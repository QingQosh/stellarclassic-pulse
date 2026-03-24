mod config;
mod db;
mod error;
mod handlers;
mod indexer;
mod middleware;
mod models;
mod routes;

use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::Config::from_env();
    let pool = db::create_pool(
        &config.database_url,
        config.db_max_connections,
        config.db_min_connections,
    )
    .await;
    db::run_migrations(&pool).await;

    info!("Migrations applied successfully");

    // Spawn background indexer
    let indexer = indexer::Indexer::new(pool.clone(), config.clone());
    tokio::spawn(async move {
        indexer.run().await;
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let router = routes::create_router(pool, config.api_key);

    info!("Soroban Pulse listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    if config.behind_proxy {
        info!("Running behind proxy — trusting X-Forwarded-For");
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    } else {
        axum::serve(listener, router).await.unwrap();
    }
}
