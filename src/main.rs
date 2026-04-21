use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::{postgres::PgStore, sqlite::SqliteStore},
    scheduler::Scheduler,
    services::{freedb::FreedBService, musicbrainz::MusicBrainzService},
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

    let rp_origin = Url::parse(&config.rp_origin)
        .context("RP_ORIGIN must be a valid URL")?;
    let webauthn = WebauthnBuilder::new(&config.rp_id, &rp_origin)
        .context("WebauthnBuilder failed")?
        .rp_name("suzuran")
        .build()
        .context("Webauthn build failed")?;

    // If ACOUSTID_KEY is set in the environment, seed it into the settings table
    // so it can be read dynamically per-job (and later managed via the UI).
    if let Ok(key) = std::env::var("ACOUSTID_KEY") {
        if !key.is_empty() {
            if let Err(e) = db.set_setting("acoustid_api_key", &key).await {
                tracing::warn!("failed to persist ACOUSTID_KEY to settings: {e}");
            } else {
                tracing::info!("ACOUSTID_KEY seeded into settings table");
            }
        }
    }
    let mb_service = Arc::new(MusicBrainzService::new());
    let freedb_service = Arc::new(FreedBService::new());

    let state = AppState::new(db.clone(), config.clone(), webauthn, mb_service.clone(), freedb_service.clone());

    // Spawn job scheduler
    let scheduler = Arc::new(Scheduler::new(db, mb_service.clone(), freedb_service.clone()));
    tokio::spawn({
        let s = scheduler.clone();
        async move { s.run().await }
    });
    tracing::info!("job scheduler started");

    let router = build_router(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
