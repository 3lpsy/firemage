use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    bytes: Vec<u8>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

#[tokio::test]
async fn catalog_http_enforces_auth_permissions_body_limits_aliases_and_references() {
    let directory = tempfile::tempdir().unwrap();
    let config = firemage_config::Server {
        data_dir: Some(directory.path().into()),
        kernel_dir: Some(directory.path().join("kernels")),
        ..Default::default()
    };
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let hash = firemage_auth::hash_password("test-password-123").unwrap();
    for (name, admin) in [("admin", true), ("reader", false)] {
        firemage_queries::add_user(&db, name.into(), Some(hash.clone()), admin, None)
            .await
            .unwrap();
    }
    let router = firemage_server::router(
        firemage_server::App::new(firemage_runtime::Runtime::new(db, config)).unwrap(),
    );
    assert_eq!(
        request(&router, "GET", "/v1/kernels", None, vec![]).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut tokens = Vec::new();
    for username in ["admin", "reader"] {
        let (_, response) = request(
            &router,
            "POST",
            "/v1/auth/login",
            None,
            json!({"username":username,"password":"test-password-123"})
                .to_string()
                .into_bytes(),
        )
        .await;
        tokens.push(response["token"].as_str().unwrap().to_owned());
    }
    let admin = Some(tokens[0].as_str());
    let reader = Some(tokens[1].as_str());
    let upload = "/v1/kernels/vmlinux/content";
    assert_eq!(
        request(&router, "PUT", upload, reader, b"kernel".to_vec())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    // A realistic kernel exceeds the global 8 MiB API request limit.
    let (status, kernel) = request(&router, "PUT", upload, admin, vec![1; 9 * 1024 * 1024]).await;
    assert_eq!(status, StatusCode::OK, "{kernel}");
    assert_eq!(kernel["size_bytes"], 9 * 1024 * 1024);
    assert_eq!(
        request(&router, "GET", "/v1/kernels", reader, vec![])
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (status, kernel) = request(
        &router,
        "PUT",
        "/v1/kernels/vmlinux",
        admin,
        json!({"alias":"Linux stable"}).to_string().into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kernel}");
    assert_eq!(kernel["alias"], "Linux stable");
    assert!(
        request(&router, "PUT", upload, admin, b"replace".to_vec())
            .await
            .0
            .is_client_error()
    );
    let (status, vm) = request(
        &router,
        "POST",
        "/v1/vms",
        admin,
        json!({"name":"review","kernel":{"kind":"kernel","name":"vmlinux"}})
            .to_string()
            .into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{vm}");
    assert_eq!(
        request(&router, "GET", "/v1/kernels", reader, vec![])
            .await
            .1[0]["vm_count"],
        1
    );
    assert!(
        request(&router, "DELETE", "/v1/kernels/vmlinux", admin, vec![])
            .await
            .0
            .is_client_error()
    );
    assert_eq!(
        request(
            &router,
            "DELETE",
            &format!("/v1/vms/{}", vm["id"].as_str().unwrap()),
            admin,
            vec![]
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&router, "DELETE", "/v1/kernels/vmlinux", admin, vec![])
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&router, "GET", "/v1/kernels", reader, vec![])
            .await
            .1,
        json!([])
    );
}
