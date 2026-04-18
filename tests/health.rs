use std::sync::Arc;
use url::Url;
use webauthn_rs::WebauthnBuilder;

use suzuran_server::{
    build_router,
    config::Config,
    dal::sqlite::SqliteStore,
    state::AppState,
};

fn test_webauthn() -> webauthn_rs::Webauthn {
    let origin = Url::parse("http://localhost:3000").unwrap();
    WebauthnBuilder::new("localhost", &origin)
        .unwrap()
        .rp_name("suzuran-test")
        .build()
        .unwrap()
}

async fn test_app() -> axum::Router {
    let store = SqliteStore::new("sqlite::memory:")
        .await
        .expect("in-memory SQLite failed");
    store.migrate().await.expect("migrations failed");

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret".into(),
        port: 0,
        log_level: "error".into(),
        rp_id: "localhost".into(),
        rp_origin: "http://localhost:3000".into(),
    };

    let state = AppState::new(Arc::new(store), config, test_webauthn());
    build_router(state)
}

#[tokio::test]
async fn health_returns_ok() {
    let app = test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let res = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}
