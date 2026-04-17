#[tokio::test]
async fn health_returns_ok() {
    // Bind to a random port to avoid conflicts
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, suzuran_server::build_router()).await.unwrap();
    });

    let res = reqwest::get(format!("http://{addr}/health"))
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "ok");
}
