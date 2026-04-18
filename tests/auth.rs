use std::sync::Arc;
use suzuran_server::{
    build_router,
    config::Config,
    dal::sqlite::SqliteStore,
    state::AppState,
};

async fn test_app() -> (axum::Router, String) {
    let store = SqliteStore::new("sqlite::memory:")
        .await
        .expect("SQLite failed");
    store.migrate().await.expect("migrations failed");

    let config = Config {
        database_url: "sqlite::memory:".into(),
        jwt_secret: "test-secret-32-chars-minimum-xxxx".into(),
        port: 0,
        log_level: "error".into(),
    };
    let base_url = format!("http://127.0.0.1"); // filled in per-test
    let state = AppState::new(Arc::new(store), config);
    (build_router(state), base_url)
}

async fn spawn_test_server() -> String {
    let (app, _) = test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn register_first_user_becomes_admin() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "alice",
            "email": "alice@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["role"], "admin");
    assert_eq!(body["username"], "alice");
    assert!(body.get("password_hash").is_none(), "password_hash must not be serialized");
}

#[tokio::test]
async fn login_sets_session_cookie() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    // Register first
    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "bob",
            "email": "bob@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    // Login
    let res = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "bob",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 204);
    assert!(
        res.headers().get("set-cookie").is_some(),
        "set-cookie header must be present"
    );
}

#[tokio::test]
async fn me_requires_authentication() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{base}/api/v1/auth/me"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn me_returns_user_after_login() {
    let base = spawn_test_server().await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    client
        .post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "username": "carol",
            "email": "carol@example.com",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "username": "carol",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    let res = client
        .get(format!("{base}/api/v1/auth/me"))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["username"], "carol");
}
