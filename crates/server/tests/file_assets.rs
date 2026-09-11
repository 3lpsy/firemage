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
async fn assets_http_enforces_owner_permissions_upload_limits_and_reference_protection() {
    let directory = tempfile::tempdir().unwrap();
    let config = firemage_config::Server {
        data_dir: Some(directory.path().into()),
        asset_dir: Some(directory.path().join("assets")),
        asset_max_bytes: Some(9 * 1024 * 1024),
        kernel_dir: Some(directory.path().join("kernels")),
        ..Default::default()
    };
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let hash = firemage_auth::hash_password("test-password-123").unwrap();
    for (name, admin) in [("admin", true), ("other", true), ("reader", false)] {
        firemage_queries::add_user(&db, name.into(), Some(hash.clone()), admin, None)
            .await
            .unwrap();
    }
    let router = firemage_server::router(
        firemage_server::App::new(firemage_runtime::Runtime::new(db, config)).unwrap(),
    );
    assert_eq!(
        request(&router, "GET", "/v1/assets", None, vec![]).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut tokens = Vec::new();
    for username in ["admin", "other", "reader"] {
        let (status, response) = request(
            &router,
            "POST",
            "/v1/auth/login",
            None,
            json!({"username": username, "password": "test-password-123"})
                .to_string()
                .into_bytes(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        tokens.push(response["token"].as_str().unwrap().to_owned());
    }
    let admin = Some(tokens[0].as_str());
    let other = Some(tokens[1].as_str());
    let reader = Some(tokens[2].as_str());
    assert_eq!(
        request(&router, "GET", "/v1/assets/limits", None, vec![])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, limits) = request(&router, "GET", "/v1/assets/limits", reader, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(limits["max_bytes"], 9 * 1024 * 1024);
    let upload = "/v1/assets?alias=review-config&filename=config.json";
    assert_eq!(
        request(&router, "POST", upload, reader, vec![]).await.0,
        StatusCode::FORBIDDEN
    );
    // Asset uploads may exceed the API's ordinary 8 MiB request limit.
    let (status, asset) = request(&router, "POST", upload, admin, vec![1; 9 * 1024 * 1024]).await;
    assert_eq!(status, StatusCode::OK, "{asset}");
    assert_eq!(asset["size_bytes"], 9 * 1024 * 1024);
    let id = asset["id"].as_str().unwrap();
    let path = format!("/v1/assets/{id}");
    assert_eq!(
        request(&router, "POST", upload, admin, vec![]).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&router, "POST", upload, other, vec![]).await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, "GET", "/v1/assets", reader, vec![])
            .await
            .1,
        json!([])
    );
    assert_eq!(
        request(&router, "GET", &format!("{path}/content"), other, vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &router,
            "PUT",
            &path,
            other,
            json!({"alias":"stolen"}).to_string().into_bytes()
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&router, "DELETE", &path, other, vec![]).await.0,
        StatusCode::NOT_FOUND
    );
    let spec = json!({"name":"review", "attachments":[{"asset_id":id,"destination":"/root/.config/opencode/opencode.json"}]}).to_string().into_bytes();
    assert_eq!(
        request(&router, "POST", "/v1/vms", other, spec.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, vm) = request(&router, "POST", "/v1/vms", admin, spec).await;
    assert_eq!(status, StatusCode::OK, "{vm}");
    assert_eq!(vm["state"], "defined");
    let (status, renamed) = request(
        &router,
        "PUT",
        &path,
        admin,
        json!({"alias":"config-v2"}).to_string().into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["id"], id);
    assert_eq!(renamed["vm_count"], 1);
    assert_eq!(
        request(&router, "DELETE", &path, admin, vec![]).await.0,
        StatusCode::BAD_REQUEST
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
        request(&router, "DELETE", &path, admin, vec![]).await.0,
        StatusCode::NO_CONTENT
    );
    assert!(!directory.path().join("assets").join(id).exists());
    let (status, error) =
        request(&router, "POST", upload, admin, vec![0; 9 * 1024 * 1024 + 1]).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{error}");
}
