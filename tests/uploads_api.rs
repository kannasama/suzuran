mod common;
use common::TestApp;

#[tokio::test]
async fn test_image_upload_and_serve() {
    let app = TestApp::spawn().await;
    let token = app.seed_admin_user().await;

    // Upload the 1×1 PNG fixture
    let png = include_bytes!("fixtures/1x1.png");
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(png.to_vec())
            .file_name("bg.png")
            .mime_str("image/png")
            .unwrap(),
    );

    let resp = app.authed_multipart(&token, "/api/v1/uploads/images", form).await;
    assert_eq!(resp.status().as_u16(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let url = body["url"].as_str().unwrap();
    assert!(
        url.starts_with("/uploads/"),
        "URL must be a local path, got: {url}"
    );
    assert!(url.ends_with(".png"), "URL must end with .png, got: {url}");

    // File must be serveable via GET
    let serve_resp = app.client
        .get(format!("{}{url}", app.addr))
        .send()
        .await
        .unwrap();
    assert_eq!(
        serve_resp.status().as_u16(),
        200,
        "expected 200 serving uploaded file at {url}"
    );
    assert_eq!(
        serve_resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
}

#[tokio::test]
async fn test_upload_rejects_non_image_mime() {
    let app = TestApp::spawn().await;
    let token = app.seed_admin_user().await;

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"not an image".to_vec())
            .file_name("evil.exe")
            .mime_str("application/octet-stream")
            .unwrap(),
    );

    let resp = app.authed_multipart(&token, "/api/v1/uploads/images", form).await;
    assert_eq!(
        resp.status().as_u16(),
        400,
        "expected 400 for non-image MIME type"
    );
}

#[tokio::test]
async fn test_upload_requires_auth() {
    let app = TestApp::spawn().await;
    let anon = reqwest::Client::new();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"data".to_vec())
            .file_name("bg.png")
            .mime_str("image/png")
            .unwrap(),
    );

    let resp = anon
        .post(format!("{}/api/v1/uploads/images", app.addr))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        401,
        "expected 401 for unauthenticated upload"
    );
}
