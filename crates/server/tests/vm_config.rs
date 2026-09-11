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
async fn config_and_address_routes_enforce_account_scope_and_mutation_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let hash = firemage_auth::hash_password("test-password-123").unwrap();
    let mut users = Vec::new();
    for (name, admin) in [("admin", true), ("owner", false), ("other", false)] {
        users.push(
            firemage_queries::add_user(&db, name.into(), Some(hash.clone()), admin, None)
                .await
                .unwrap(),
        );
    }
    let runtime = firemage_runtime::Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    runtime
        .secrets()
        .await
        .unwrap()
        .put(&users[1].id, "auth-file", "private-file-value")
        .await
        .unwrap();
    let vm = runtime.define(&users[1].id, serde_json::from_value(json!({
        "name":"owned", "secret_attachments":[{"secret":"auth-file","destination":"/root/auth.json"}]
    })).unwrap()).await.unwrap();
    firemage_queries::insert_network(
        &db,
        &users[1].id,
        "owned-net",
        json!({"name":"owned-net","subnet":"10.71.0.0/29","gateway":"10.71.0.1"}).to_string(),
    )
    .await
    .unwrap();
    let router = firemage_server::router(firemage_server::App::new(runtime).unwrap());
    let mut tokens = Vec::new();
    for name in ["admin", "owner", "other"] {
        let (status, value) = request(
            &router,
            "POST",
            "/v1/auth/login",
            None,
            json!({"username":name,"password":"test-password-123"})
                .to_string()
                .into_bytes(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        tokens.push(value["token"].as_str().unwrap().to_owned());
    }
    let admin = Some(tokens[0].as_str());
    let owner = Some(tokens[1].as_str());
    let other = Some(tokens[2].as_str());
    let path = format!("/v1/vms/{}/config", vm.id);
    assert_eq!(
        request(&router, "GET", &path, None, vec![]).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, value) = request(&router, "GET", &path, owner, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    let source = value["toml"].as_str().unwrap();
    assert!(source.contains("auth-file"));
    assert!(!source.contains("private-file-value"));
    assert_eq!(
        request(&router, "GET", &path, admin, vec![]).await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, "GET", &path, other, vec![]).await.0,
        StatusCode::NOT_FOUND
    );
    let body = json!({"toml":source,"name":"renamed"})
        .to_string()
        .into_bytes();
    for route in [
        "/v1/vm-config/preview".to_owned(),
        "/v1/vm-config/import".into(),
        format!("{path}/preview"),
    ] {
        assert_eq!(
            request(&router, "POST", &route, owner, body.clone())
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        request(&router, "PUT", &path, owner, body.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &router,
            "POST",
            &format!("{path}/preview"),
            admin,
            body.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, updated) = request(&router, "PUT", &path, admin, body).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["spec"]["name"], "renamed");
    let (status, preview) = request(
        &router,
        "GET",
        "/v1/networks/owned-net/suggestion",
        owner,
        vec![],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["address"], "10.71.0.2");
    assert_eq!(
        request(
            &router,
            "GET",
            "/v1/networks/owned-net/suggestion",
            other,
            vec![]
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let route = format!("/v1/networks/owned-net/suggestion?vm={}", vm.id);
    assert_eq!(
        request(&router, "GET", &route, admin, vec![]).await.0,
        StatusCode::OK
    );
    assert_eq!(
        request(&router, "GET", &route, other, vec![]).await.0,
        StatusCode::NOT_FOUND
    );
}
