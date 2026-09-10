use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

struct Fixture {
    router: Router,
    _directory: tempfile::TempDir,
}
impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
        let hash = firemage_auth::hash_password("test-password-123").unwrap();
        for (name, admin) in [("alice", true), ("bob", true), ("reader", false)] {
            firemage_queries::add_user(&db, name.into(), Some(hash.clone()), admin, None)
                .await
                .unwrap();
        }
        let config = firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        };
        let runtime = firemage_runtime::Runtime::new(db, config);
        Self {
            router: firemage_server::router(firemage_server::App::new(runtime).unwrap()),
            _directory: directory,
        }
    }
    async fn request(
        &self,
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
        let response = self
            .router
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
    async fn login(&self, name: &str) -> String {
        let (status, body) = self
            .request(
                "POST",
                "/v1/auth/login",
                None,
                json!({"username":name,"password":"test-password-123"}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["token"].as_str().unwrap().into()
    }
}

#[tokio::test]
async fn login_rotation_and_revocation() {
    let f = Fixture::new().await;
    assert_eq!(
        f.request("GET", "/v1/me", None, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    for name in ["alice", "missing"] {
        let (status, body) = f
            .request(
                "POST",
                "/v1/auth/login",
                None,
                json!({"username":name,"password":"wrong"}),
            )
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "invalid username or password");
    }
    let token = f.login("alice").await;
    let (left, right) = tokio::join!(
        f.request("POST", "/v1/auth/refresh", Some(&token), Value::Null),
        f.request("POST", "/v1/auth/refresh", Some(&token), Value::Null)
    );
    assert_ne!(left.0.is_success(), right.0.is_success());
    let winner = if left.0.is_success() { left.1 } else { right.1 };
    let new_token = winner["token"].as_str().unwrap();
    assert_ne!(token, new_token);
    assert_eq!(
        f.request("GET", "/v1/me", Some(&token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request("GET", "/v1/me", Some(new_token), Value::Null)
            .await
            .1["username"],
        "alice"
    );
    assert_eq!(
        f.request("POST", "/v1/auth/logout", Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.request("GET", "/v1/me", Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn api_tokens_are_expiring_owned_and_cannot_mint_credentials() {
    let f = Fixture::new().await;
    let alice = f.login("alice").await;
    let bob = f.login("bob").await;
    let input = json!({"name":"runner","expires_at":firemage_queries::now() + 3600});
    assert_eq!(
        f.request(
            "POST",
            "/v1/apitokens",
            Some(&alice),
            json!({"name":"expired","expires_at":1})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, result) = f
        .request("POST", "/v1/apitokens", Some(&alice), input.clone())
        .await;
    assert_eq!(status, StatusCode::OK);
    let api = result["token"].as_str().unwrap();
    assert_eq!(
        f.request("GET", "/v1/me", Some(api), Value::Null).await.1["username"],
        "alice"
    );
    assert_eq!(
        f.request("POST", "/v1/auth/refresh", Some(api), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request("POST", "/v1/apitokens", Some(api), input).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, list) = f
        .request("GET", "/v1/apitokens", Some(&alice), Value::Null)
        .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(list[0].get("token").is_none());
    assert!(list[0].get("token_hash").is_none());
    let path = format!("/v1/apitokens/{}", list[0]["id"].as_str().unwrap());
    assert_eq!(
        f.request("GET", "/v1/apitokens", Some(&bob), Value::Null)
            .await
            .1,
        json!([])
    );
    assert_eq!(
        f.request("DELETE", &path, Some(&bob), Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        f.request("GET", "/v1/me", Some(api), Value::Null).await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("DELETE", &path, Some(&alice), Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        f.request("GET", "/v1/me", Some(api), Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn vm_ownership_and_host_permissions_are_enforced() {
    let f = Fixture::new().await;
    let alice = f.login("alice").await;
    let bob = f.login("bob").await;
    let reader = f.login("reader").await;
    let spec = json!({"name":"job"});
    assert_eq!(
        f.request("POST", "/v1/vms", Some(&reader), spec.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request("GET", "/v1/users", Some(&reader), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, vm) = f.request("POST", "/v1/vms", Some(&alice), spec).await;
    assert_eq!(status, StatusCode::OK, "{vm}");
    let path = format!("/v1/vms/{}", vm["id"].as_str().unwrap());
    assert_eq!(
        f.request("GET", &path, Some(&alice), Value::Null).await.0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("GET", "/v1/vms", Some(&reader), Value::Null)
            .await
            .1,
        json!([])
    );
    assert_eq!(
        f.request("GET", &path, Some(&bob), Value::Null).await.0,
        StatusCode::OK
    );
    for (method, suffix, body) in [
        ("GET", "", Value::Null),
        ("DELETE", "", Value::Null),
        ("GET", "/logs", Value::Null),
        ("GET", "/files?path=result.txt", Value::Null),
        ("POST", "/actions", json!({"action":"stop"})),
        ("POST", "/firecracker", json!({"method":"GET","path":"/"})),
    ] {
        let (status, body) = f
            .request(method, &format!("{path}{suffix}"), Some(&reader), body)
            .await;
        assert!(status.is_client_error(), "{method} {suffix}: {status}");
        assert!(
            body["error"] == "VM not found"
                || body["error"] == "administrator required for host resource access"
        );
    }
    assert_eq!(
        f.request("GET", &path, Some(&alice), Value::Null).await.1["state"],
        "defined"
    );
}

#[tokio::test]
async fn management_rejects_readers_and_edits_defined_vms() {
    let f = Fixture::new().await;
    let alice = f.login("alice").await;
    let reader = f.login("reader").await;
    for path in ["/v1/config", "/v1/users"] {
        assert_eq!(
            f.request("GET", path, Some(&reader), Value::Null).await.0,
            StatusCode::FORBIDDEN
        );
    }
    for (method, path) in [("POST", "/v1/config/validate"), ("PUT", "/v1/config")] {
        assert_eq!(
            f.request(
                method,
                path,
                Some(&reader),
                json!({"toml":"[server]", "revision":"stale"})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        f.request("GET", "/v1/host", Some(&reader), Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    let (_, vm) = f
        .request("POST", "/v1/vms", Some(&alice), json!({"name":"before"}))
        .await;
    let path = format!("/v1/vms/{}", vm["id"].as_str().unwrap());
    assert_eq!(
        f.request("PUT", &path, Some(&reader), json!({"name":"hijack"}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, vm) = f
        .request(
            "PUT",
            &path,
            Some(&alice),
            json!({"name":"after","vcpus":2}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{vm}");
    assert_eq!(vm["spec"]["name"], "after");
    assert_eq!(vm["spec"]["vcpus"], 2);
    let (_, activity) = f
        .request("GET", "/v1/activity", Some(&alice), Value::Null)
        .await;
    assert!(
        activity
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "vm.update")
    );
}

#[tokio::test]
async fn disabling_users_revokes_sessions_and_blocks_login() {
    let f = Fixture::new().await;
    let alice = f.login("alice").await;
    let bob = f.login("bob").await;
    let (_, users) = f
        .request("GET", "/v1/users", Some(&alice), Value::Null)
        .await;
    let user = users
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "bob")
        .unwrap();
    let path = format!("/v1/users/{}", user["id"].as_str().unwrap());
    let (status, body) = f
        .request(
            "PUT",
            &path,
            Some(&alice),
            json!({"username":"bob","admin":true,"disabled":true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        f.request("GET", "/v1/me", Some(&bob), Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request(
            "POST",
            "/v1/auth/login",
            None,
            json!({"username":"bob","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let user = users
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "alice")
        .unwrap();
    let path = format!("/v1/users/{}", user["id"].as_str().unwrap());
    assert_eq!(
        f.request("DELETE", &path, Some(&alice), Value::Null)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn activity_visibility_follows_user_id_when_names_are_reused() {
    let f = Fixture::new().await;
    let admin = f.login("alice").await;
    let reader = f.login("reader").await;
    let created = f
        .request(
            "POST",
            "/v1/apitokens",
            Some(&reader),
            json!({"name":"private-history","expires_at":firemage_queries::now()+600}),
        )
        .await;
    assert_eq!(created.0, StatusCode::OK);
    let rows = f
        .request("GET", "/v1/users", Some(&admin), Value::Null)
        .await
        .1;
    let id = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["username"] == "reader")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        f.request(
            "PUT",
            &format!("/v1/users/{id}"),
            Some(&admin),
            json!({"username":"renamed-reader","admin":false,"disabled":false})
        )
        .await
        .0,
        StatusCode::OK
    );
    let retained = f
        .request("GET", "/v1/activity", Some(&reader), Value::Null)
        .await
        .1;
    assert!(
        retained
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["resource"] == "private-history")
    );
    assert_eq!(
        f.request(
            "POST",
            "/v1/users",
            Some(&admin),
            json!({"username":"reader","password":"test-password-123"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let replacement = f.login("reader").await;
    let rows = f
        .request("GET", "/v1/activity", Some(&replacement), Value::Null)
        .await
        .1;
    assert_eq!(rows, json!([]));
}

#[tokio::test]
async fn secrets_are_write_only_owner_scoped_and_session_only() {
    let f = Fixture::new().await;
    let alice = f.login("alice").await;
    let bob = f.login("bob").await;
    let reader = f.login("reader").await;
    let (status, metadata) = f
        .request(
            "PUT",
            "/v1/secrets/service-key",
            Some(&alice),
            json!({"value":"do-not-return"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{metadata}");
    assert_eq!(metadata["name"], "service-key");
    assert!(!metadata.to_string().contains("do-not-return"));
    let (_, list) = f
        .request("GET", "/v1/secrets", Some(&alice), Value::Null)
        .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains("do-not-return"));
    let (_, list) = f
        .request("GET", "/v1/secrets", Some(&bob), Value::Null)
        .await;
    assert!(list.as_array().unwrap().is_empty());
    assert_eq!(
        f.request("DELETE", "/v1/secrets/service-key", Some(&bob), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        f.request("GET", "/v1/secrets", Some(&reader), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (_, token) = f
        .request(
            "POST",
            "/v1/apitokens",
            Some(&alice),
            json!({"name":"automation","expires_at":firemage_queries::now()+3600}),
        )
        .await;
    assert_eq!(
        f.request("GET", "/v1/secrets", token["token"].as_str(), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        f.request("GET", "/v1/secrets/service-key", Some(&alice), Value::Null)
            .await
            .0,
        StatusCode::METHOD_NOT_ALLOWED
    );
    assert_eq!(
        f.request(
            "DELETE",
            "/v1/secrets/service-key",
            Some(&alice),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
}
