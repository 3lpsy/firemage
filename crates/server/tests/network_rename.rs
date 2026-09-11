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
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
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

fn network(name: &str) -> Value {
    json!({"name":name,"subnet":"10.99.1.0/24","gateway":"10.99.1.1","policy":{"mode":"isolated"}})
}

#[tokio::test]
async fn rename_keeps_uuid_and_live_vm_configuration_while_protecting_in_use_network_settings() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let mut owners = Vec::new();
    for (name, admin) in [("owner", true), ("other", true), ("reader", false)] {
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
    let router = firemage_server::router(
        firemage_server::App::new(firemage_runtime::Runtime::new(
            db.clone(),
            firemage_config::Server {
                data_dir: Some(directory.path().into()),
                ..Default::default()
            },
        ))
        .unwrap(),
    );
    assert_eq!(
        request(&router, "GET", "/v1/networks", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for (token, name) in [("owner", "original"), ("other", "foreign")] {
        let (status, response) =
            request(&router, "POST", "/v1/networks", Some(token), network(name)).await;
        assert_eq!(status, StatusCode::OK, "{response}");
    }
    let list = request(&router, "GET", "/v1/networks", Some("owner"), Value::Null)
        .await
        .1;
    assert_eq!(list.as_array().unwrap().len(), 2);
    for item in list.as_array().unwrap() {
        assert_eq!(item["id"].as_str().unwrap().len(), 36);
    }
    let own = firemage_queries::network(&db, &owners[0], "original")
        .await
        .unwrap();
    let foreign = firemage_queries::network(&db, &owners[1], "foreign")
        .await
        .unwrap();
    assert_ne!(own.id, foreign.id);
    let mut before = Vec::new();
    for (index, state) in [(10, "running"), (11, "stopped")] {
        let reference = if index == 10 { "original" } else { &own.id };
        let (status, vm) = request(&router, "POST", "/v1/vms", Some("owner"), json!({
            "name":format!("vm-{state}"),"network":{"network":reference,"address":format!("10.99.1.{index}"),"mac":format!("02:00:00:00:00:{index:02x}")}
        })).await;
        assert_eq!(status, StatusCode::OK, "{vm}");
        assert_eq!(vm["spec"]["network"]["network"], own.id);
        let row = firemage_queries::vm(&db, &owners[0], vm["id"].as_str().unwrap())
            .await
            .unwrap();
        before.push(
            firemage_queries::set_vm_state(&db, row, state, None, None)
                .await
                .unwrap(),
        );
    }
    let path = format!("/v1/networks/{}", own.id);
    assert_eq!(
        request(&router, "PUT", &path, Some("reader"), network("renamed"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, response) =
        request(&router, "PUT", &path, Some("owner"), network("renamed")).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let renamed = firemage_queries::network(&db, &owners[0], &own.id)
        .await
        .unwrap();
    assert_eq!(renamed.name, "renamed");
    assert_eq!(
        serde_json::from_str::<Value>(&renamed.spec).unwrap()["name"],
        "renamed"
    );
    assert_eq!(
        firemage_queries::network(&db, &owners[1], &foreign.id)
            .await
            .unwrap(),
        foreign
    );
    for expected in before {
        assert_eq!(
            firemage_queries::vm(&db, &owners[0], &expected.id)
                .await
                .unwrap(),
            expected
        );
    }
    assert!(
        request(&router, "DELETE", &path, Some("owner"), Value::Null)
            .await
            .0
            .is_client_error()
    );
    for changed in [
        json!({"subnet":"10.99.0.0/16"}),
        json!({"gateway":"10.99.1.2"}),
        json!({"policy":{"mode":"unrestricted"}}),
    ] {
        let mut spec = network("renamed");
        spec.as_object_mut()
            .unwrap()
            .extend(changed.as_object().unwrap().clone());
        assert!(
            request(&router, "PUT", &path, Some("owner"), spec)
                .await
                .0
                .is_client_error()
        );
    }
    assert_eq!(
        request(
            &router,
            "POST",
            "/v1/networks",
            Some("owner"),
            network("occupied")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(
        request(&router, "PUT", &path, Some("owner"), network("occupied"))
            .await
            .0
            .is_client_error()
    );
    assert_eq!(
        firemage_queries::network(&db, &owners[0], &own.id)
            .await
            .unwrap()
            .name,
        "renamed"
    );
}
