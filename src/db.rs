use sqlx::{postgres::PgPoolOptions, Executor, PgPool};
use tracing::info;

pub async fn create_pool(
    database_url: &str,
    db_max_connections: u32,
    db_min_connections: u32,
    db_statement_timeout_ms: u64,
) -> Result<PgPool, sqlx::Error> {
    info!(
        min_connections = db_min_connections,
        max_connections = db_max_connections,
        statement_timeout_ms = db_statement_timeout_ms,
        "Configuring Postgres connection pool"
    );

    PgPoolOptions::new()
        .max_connections(db_max_connections)
        .min_connections(db_min_connections)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                conn.execute(
                    format!("SET statement_timeout = '{db_statement_timeout_ms}ms'").as_str(),
                )
                .await
                .map(|_| ())
            })
        })
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
