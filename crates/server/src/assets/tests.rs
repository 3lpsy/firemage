use crate::{App, router};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;

#[tokio::test]
async fn spa_assets_keep_api_paths_reserved_and_reject_symlink_escape() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("index.html"), "<html>firemage</html>").unwrap();
    std::fs::write(directory.path().join("app.wasm"), [0, 97, 115, 109]).unwrap();
    std::fs::write(outside.path().join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret"),
        directory.path().join("secret.txt"),
    )
    .unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let config = firemage_config::Server {
        webui_dir: Some(directory.path().into()),
        ..Default::default()
    };
    let router = router(App::new(firemage_runtime::Runtime::new(db, config)).unwrap());
    for path in ["/", "/vms/123", "/networks"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    }
    for path in ["/v1", "/v1/missing", "/missing.js", "/secret.txt"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert!(
            response.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("application/json")
        );
    }
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/%2e%2e/secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = router
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/app.wasm")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/wasm");
    assert!(to_bytes(response.into_body(), 10).await.unwrap().is_empty());
}
