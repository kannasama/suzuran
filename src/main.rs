use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use suzuran_server::{
    build_router,
    config::Config,
    dal::{postgres::PgStore, sqlite::SqliteStore},
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let config = Config::from_env()?;

    tracing_subscriber::registry()
        .with(EnvFilter::new(&config.log_level))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db: Arc<dyn suzuran_server::dal::Store> =
        if config.database_url.starts_with("postgres") {
            let store = PgStore::new(&config.database_url)
                .await
                .context("failed to connect to Postgres")?;
            store.migrate().await.context("Postgres migrations failed")?;
            tracing::info!("Postgres migrations applied");
            Arc::new(store)
        } else {
            let store = SqliteStore::new(&config.database_url)
                .await
                .context("failed to connect to SQLite")?;
            store.migrate().await.context("SQLite migrations failed")?;
            tracing::info!("SQLite migrations applied");
            Arc::new(store)
        };

    let state = AppState::new(db, config.clone());
    let router = build_router(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
