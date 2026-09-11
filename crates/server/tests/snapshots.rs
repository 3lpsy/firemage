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
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    (
        response.status(),
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}
fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[tokio::test]
async fn snapshot_routes_authenticate_scope_downloads_and_require_admin_mutations() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let hash = firemage_auth::hash_password("test-password-123").unwrap();
    let mut users = Vec::new();
    for (name, admin) in [("owner", true), ("other", true), ("reader", false)] {
        users.push(
            firemage_queries::add_user(&db, name.into(), Some(hash.clone()), admin, None)
                .await
                .unwrap(),
        );
    }
    let runtime = firemage_runtime::Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    let router = firemage_server::router(firemage_server::App::new(runtime.clone()).unwrap());
    let mut tokens = Vec::new();
    for username in ["owner", "other", "reader"] {
        let (status, bytes) = request(
            &router,
            "POST",
            "/v1/auth/login",
            None,
            json!({"username":username,"password":"test-password-123"})
                .to_string()
                .into_bytes(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        tokens.push(json(&bytes)["token"].as_str().unwrap().to_owned());
    }
    let owner = Some(tokens[0].as_str());
    let other = Some(tokens[1].as_str());
    let reader = Some(tokens[2].as_str());
    assert_eq!(
        request(&router, "GET", "/v1/snapshots", None, vec![])
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &router,
            "POST",
            "/v1/snapshots?alias=sample",
            reader,
            vec![]
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let stage = tempfile::tempdir().unwrap();
    for name in ["state.bin", "kernel", "rootfs.ext4"] {
        std::fs::write(stage.path().join(name), name).unwrap();
    }
    firemage_snapshots::create_private(&stage.path().join("memory.bin"))
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let files = ["state.bin", "memory.bin", "kernel", "rootfs.ext4"]
        .into_iter()
        .map(|name| (name.to_owned(), json!({"size_bytes":0,"sha256":""})))
        .collect::<serde_json::Map<_, _>>();
    let manifest=serde_json::from_value(json!({"version":1,"source_vm_name":"source","architecture":std::env::consts::ARCH,"firecracker_version":"1.16.1","spec":{"name":"source","memory_mib":64},"network":null,"gateway_mac":null,"files":files})).unwrap();
    let bundle = stage.path().join("test.fmsnap");
    firemage_snapshots::pack(stage.path(), manifest, &bundle, 128 * 1024 * 1024).unwrap();
    let bytes = std::fs::read(bundle).unwrap();
    let (status, response) = request(
        &router,
        "POST",
        "/v1/snapshots?alias=uploaded",
        owner,
        bytes.clone(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&response)
    );
    let snapshot = json(&response);
    let id = snapshot["id"].as_str().unwrap();
    assert_eq!(snapshot["trusted"], false);
    assert_eq!(snapshot["owner_id"], users[0].id);
    assert!(snapshot["source_vm_id"].is_null());
    let listed = json(
        &request(&router, "GET", "/v1/snapshots", other, vec![])
            .await
            .1,
    );
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], id);
    assert_eq!(
        json(
            &request(&router, "GET", "/v1/snapshots", reader, vec![])
                .await
                .1
        ),
        json!([])
    );
    let path = format!("/v1/snapshots/{id}");
    assert_eq!(
        request(&router, "GET", &format!("{path}/download"), reader, vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    for (method, route) in [("POST", format!("{path}/trust")), ("DELETE", path.clone())] {
        assert_eq!(
            request(&router, method, &route, reader, vec![]).await.0,
            StatusCode::FORBIDDEN
        );
    }
    let (status, download) =
        request(&router, "GET", &format!("{path}/download"), other, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(download, bytes);
    let target = runtime
        .define(
            &users[2].id,
            serde_json::from_value(json!({"name":"reader-target", "memory_mib":128})).unwrap(),
        )
        .await
        .unwrap();
    let restore = format!("/v1/vms/{}/snapshots/restore", target.id);
    let input = json!({"snapshot_id":id}).to_string().into_bytes();
    assert_eq!(
        request(&router, "POST", &restore, reader, input.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, response) = request(&router, "POST", &restore, other, input.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json(&response)["error"]
            .as_str()
            .unwrap()
            .contains("must be trusted")
    );
    assert_eq!(
        json(
            &request(&router, "POST", &format!("{path}/trust"), other, vec![])
                .await
                .1
        )["trusted"],
        true
    );
    let (status, response) = request(&router, "POST", &restore, other, input).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json(&response)["error"]
            .as_str()
            .unwrap()
            .contains("CPU or memory differs")
    );
    assert_eq!(
        firemage_queries::vm(&runtime.db, &users[2].id, &target.id)
            .await
            .unwrap()
            .state,
        "defined"
    );
    assert_eq!(
        request(&router, "DELETE", &path, other, vec![]).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&router, "GET", &format!("{path}/download"), owner, vec![])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let archive = stage.path().join("reader.fmsnap");
    std::fs::write(&archive, &bytes).unwrap();
    let reader_snapshot = runtime
        .import_snapshot(
            &users[2].id,
            firemage_wire::SnapshotUpload {
                alias: "reader-snapshot".into(),
                trusted: false,
            },
            &archive,
        )
        .await
        .unwrap();
    let listed = json(
        &request(&router, "GET", "/v1/snapshots", reader, vec![])
            .await
            .1,
    );
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], reader_snapshot.id);
    let (status, download) = request(
        &router,
        "GET",
        &format!("/v1/snapshots/{}/download", reader_snapshot.id),
        reader,
        vec![],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(download, bytes);
    let mut limited = runtime;
    limited.config.snapshot_max_bytes = Some(8);
    let router = firemage_server::router(firemage_server::App::new(limited).unwrap());
    assert_eq!(
        request(
            &router,
            "POST",
            "/v1/snapshots?alias=large",
            owner,
            vec![0; 9]
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
