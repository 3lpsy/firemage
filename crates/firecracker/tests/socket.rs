use axum::{
    Json, Router,
    http::StatusCode,
    routing::{get, put},
};
use firemage_firecracker::Firecracker;
use serde_json::{Value, json};

#[tokio::test]
async fn unix_socket_preserves_payloads_status_and_empty_responses() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let socket = directory.path().join("fc.sock");
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    let router = Router::new()
        .route("/", get(|| async { Json(json!({"state":"Running"})) }))
        .route(
            "/machine-config",
            put(|Json(value): Json<Value>| async move {
                assert_eq!(value, json!({"vcpu_count":2,"mem_size_mib":128}));
                StatusCode::NO_CONTENT
            }),
        )
        .route(
            "/actions",
            put(|| async {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"fault_message":"invalid state"})),
                )
            }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Firecracker::new(&socket).unwrap();
    assert_eq!(client.state().await.unwrap(), "Running");
    assert_eq!(
        client
            .request(
                "PUT",
                "/machine-config",
                Some(&json!({"vcpu_count":2,"mem_size_mib":128}))
            )
            .await
            .unwrap(),
        (204, Value::Null)
    );
    let (status, body) = client
        .request(
            "PUT",
            "/actions",
            Some(&json!({"action_type":"InstanceStart"})),
        )
        .await
        .unwrap();
    assert_eq!(status, 400);
    assert_eq!(body["fault_message"], "invalid state");
    assert!(
        client
            .call("PUT", "/actions", Value::Null)
            .await
            .unwrap_err()
            .to_string()
            .contains("Firecracker HTTP 400")
    );
    server.abort();
}

#[tokio::test]
async fn rejects_paths_that_can_escape_the_firecracker_endpoint() {
    let client =
        Firecracker::new(std::path::Path::new("/nonexistent-firecracker-test.sock")).unwrap();
    for path in [
        "http://example.test/",
        "//example.test/",
        "/../actions",
        "/%2e%2e/actions",
        "/actions?next=x",
        "/actions#fragment",
        "/a\\b",
    ] {
        let error = client.request("GET", path, None).await.unwrap_err();
        assert_eq!(error.to_string(), "invalid Firecracker API path", "{path}");
    }
    assert_eq!(
        client
            .request("DELETE", "/", None)
            .await
            .unwrap_err()
            .to_string(),
        "Firecracker method must be GET, PUT or PATCH"
    );
}
