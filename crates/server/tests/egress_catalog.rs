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
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
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
struct Fixture {
    _directory: tempfile::TempDir,
    db: firemage_queries::DatabaseConnection,
    router: Router,
    owners: Vec<String>,
}
async fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let mut owners = Vec::new();
    for (name, admin) in [("admin", true), ("other", true), ("reader", false)] {
        let owner = firemage_queries::add_user(&db, name.into(), None, admin, None)
            .await
            .unwrap();
        firemage_queries::insert_credential(
            &db,
            &owner.id,
            firemage_auth::token_hash(name),
            "session",
            "test",
            firemage_queries::now() + 600,
        )
        .await
        .unwrap();
        owners.push(owner.id);
    }
    let config = firemage_config::Server {
        data_dir: Some(directory.path().into()),
        ..Default::default()
    };
    let router = firemage_server::router(
        firemage_server::App::new(firemage_runtime::Runtime::new(db.clone(), config)).unwrap(),
    );
    Fixture {
        _directory: directory,
        db,
        router,
        owners,
    }
}
fn policy(alias: &str, proxy: Option<&str>) -> Value {
    json!({"alias":alias,"policy":{"inherit_upstream":false},"upstream_proxy_id":proxy})
}
fn proxy(alias: &str) -> Value {
    json!({"alias":alias,"proxy":{"url":"http://proxy.example:3128"}})
}

#[tokio::test]
async fn catalog_routes_enforce_auth_ownership_and_optimistic_updates() {
    let f = fixture().await;
    for path in ["/v1/egress/policies", "/v1/egress/proxies"] {
        assert_eq!(
            request(&f.router, "GET", path, None, Value::Null).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            &f.router,
            "POST",
            "/v1/egress/policies",
            Some("reader"),
            policy("review", None)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &f.router,
            "POST",
            "/v1/egress/proxies",
            Some("reader"),
            proxy("proxy")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, created) = request(
        &f.router,
        "POST",
        "/v1/egress/proxies",
        Some("admin"),
        proxy("proxy"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let proxy_id = created["id"].as_str().unwrap();
    let input = policy("review", Some(proxy_id));
    assert_eq!(
        request(
            &f.router,
            "POST",
            "/v1/egress/policies",
            Some("other"),
            input.clone()
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, created) = request(
        &f.router,
        "POST",
        "/v1/egress/policies",
        Some("admin"),
        input.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().unwrap();
    let path = format!("/v1/egress/policies/{id}");
    assert_eq!(
        request(&f.router, "GET", &path, Some("reader"), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &f.router,
            "GET",
            "/v1/egress/policies",
            Some("reader"),
            Value::Null
        )
        .await
        .1,
        json!([])
    );
    assert_eq!(
        request(&f.router, "GET", &path, Some("other"), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let mut update = input;
    update["revision"] = 1.into();
    update["alias"] = "updated".into();
    let (status, saved) = request(&f.router, "PUT", &path, Some("admin"), update.clone()).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["revision"], 2);
    assert_eq!(
        request(&f.router, "PUT", &path, Some("admin"), update)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &f.router,
            "DELETE",
            &format!("/v1/egress/proxies/{proxy_id}"),
            Some("admin"),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&f.router, "DELETE", &path, Some("admin"), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &f.router,
            "DELETE",
            &format!("/v1/egress/proxies/{proxy_id}"),
            Some("admin"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn catalog_details_report_vm_references_and_protect_in_use_deletion() {
    let f = fixture().await;
    let (_, upstream) = request(
        &f.router,
        "POST",
        "/v1/egress/proxies",
        Some("admin"),
        proxy("proxy"),
    )
    .await;
    let proxy_id = upstream["id"].as_str().unwrap();
    let (_, created) = request(
        &f.router,
        "POST",
        "/v1/egress/policies",
        Some("admin"),
        policy("review", Some(proxy_id)),
    )
    .await;
    let id = created["id"].as_str().unwrap();
    let vm = firemage_queries::insert_vm(
        &f.db,
        &f.owners[0],
        "review-vm",
        json!({"name":"review-vm","egress_policy":id}).to_string(),
        "/tmp/review.sock".into(),
    )
    .await
    .unwrap();
    let path = format!("/v1/egress/policies/{id}");
    let (_, detail) = request(&f.router, "GET", &path, Some("admin"), Value::Null).await;
    assert_eq!(detail["vm_count"], 1);
    assert_eq!(detail["vms"][0]["id"], vm.id);
    assert_eq!(detail["vms"][0]["owner_id"], f.owners[0]);
    let (_, detail) = request(
        &f.router,
        "GET",
        &format!("/v1/egress/proxies/{proxy_id}"),
        Some("admin"),
        Value::Null,
    )
    .await;
    assert_eq!(detail["policy_count"], 1);
    assert_eq!(detail["vm_count"], 1);
    assert_eq!(detail["policies"][0]["id"], id);
    assert_eq!(
        request(&f.router, "DELETE", &path, Some("admin"), Value::Null)
            .await
            .0,
        StatusCode::CONFLICT
    );
    firemage_queries::delete_vm(&f.db, &f.owners[0], &vm.id)
        .await
        .unwrap();
    assert_eq!(
        request(&f.router, "DELETE", &path, Some("admin"), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn catalog_credentials_roundtrip_as_references_and_proxy_updates_check_revision() {
    let f = fixture().await;
    let secret_value = "credential-value-that-must-never-appear-in-catalog-responses";
    assert_eq!(
        request(
            &f.router,
            "PUT",
            "/v1/secrets/proxy-password",
            Some("admin"),
            json!({"value":secret_value})
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut input = proxy("authenticated");
    input["proxy"]["username"] = "review".into();
    input["proxy"]["password"] = json!({"secret":"proxy-password"});
    assert_ne!(
        request(
            &f.router,
            "POST",
            "/v1/egress/proxies",
            Some("other"),
            input.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, created) = request(
        &f.router,
        "POST",
        "/v1/egress/proxies",
        Some("admin"),
        input.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["proxy"]["password"]["secret"], "proxy-password");
    assert!(!created.to_string().contains(secret_value));
    let path = format!("/v1/egress/proxies/{}", created["id"].as_str().unwrap());
    input["revision"] = 1.into();
    input["proxy"]["url"] = "http://proxy.example:8080".into();
    let (status, updated) = request(&f.router, "PUT", &path, Some("admin"), input.clone()).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["revision"], 2);
    assert_eq!(
        request(&f.router, "PUT", &path, Some("admin"), input)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (_, listed) = request(
        &f.router,
        "GET",
        "/v1/egress/proxies",
        Some("admin"),
        Value::Null,
    )
    .await;
    assert!(!listed.to_string().contains(secret_value));
    let mut input = policy("signed", Some(created["id"].as_str().unwrap()));
    input["policy"]["http"] = json!({"rules":[{"host":"api.example","port":443,"scheme":"https","headers":{"Authorization":{"secret":"proxy-password","prefix":"Bearer "}}}]});
    let (status, created) = request(
        &f.router,
        "POST",
        "/v1/egress/policies",
        Some("admin"),
        input,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(
        created["policy"]["http"]["rules"][0]["headers"]["Authorization"]["secret"],
        "proxy-password"
    );
    assert!(!created.to_string().contains(secret_value));
}
