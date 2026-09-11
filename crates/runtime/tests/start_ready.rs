use axum::{
    Json, Router,
    extract::{Request, State},
    response::IntoResponse,
};
use firemage_runtime::Runtime;
use firemage_wire::{VmAction, VmState};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct Fake {
    configured: bool,
    requests: Arc<Mutex<Vec<String>>>,
}
async fn handle(State(fake): State<Fake>, request: Request) -> impl IntoResponse {
    let route = format!("{} {}", request.method(), request.uri().path());
    fake.requests.lock().await.push(route.clone());
    Json(if route == "GET /vm/config" {
        if fake.configured {
            json!({"boot-source":{"kernel_image_path":"/manual-kernel"},"drives":[{"is_root_device":true}]})
        } else {
            json!({"boot-source":null,"drives":[]})
        }
    } else {
        serde_json::Value::Null
    })
}

#[tokio::test]
async fn start_prepares_manually_launched_vm_and_preserves_already_configured_vm() {
    for configured in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("api.sock");
        let fake = Fake {
            configured,
            requests: Arc::default(),
        };
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let app = Router::new().fallback(handle).with_state(fake.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
        let user = firemage_queries::add_user(&db, "admin".into(), None, true, None)
            .await
            .unwrap();
        let kernel_dir = dir.path().join("kernels");
        std::fs::create_dir(&kernel_dir).unwrap();
        std::fs::write(kernel_dir.join("kernel"), b"kernel").unwrap();
        let runtime = Runtime::new(
            db.clone(),
            firemage_config::Server {
                data_dir: Some(dir.path().into()),
                kernel_dir: Some(kernel_dir),
                allow_trusted_vms: Some(true),
                ..Default::default()
            },
        );
        let row = firemage_queries::insert_vm(&db,&user.id,"manual",json!({"name":"manual","kernel":{"kind":"kernel","name":"kernel"},"security":{"mode":"trusted"}}).to_string(),socket.to_str().unwrap().into()).await.unwrap();
        std::fs::create_dir_all(runtime.directory(&row.id)).unwrap();
        std::fs::write(runtime.directory(&row.id).join("rootfs.ext4"), b"root disk").unwrap();
        let row = firemage_queries::set_vm_state(&db, row, "ready", None, None)
            .await
            .unwrap();
        let vm = runtime
            .action(&user.id, &row.id, VmAction::Start)
            .await
            .unwrap();
        assert_eq!(vm.state, VmState::Running);
        let requests = fake.requests.lock().await.clone();
        assert_eq!(requests.last().unwrap(), "PUT /actions");
        if configured {
            assert_eq!(requests, vec!["GET /vm/config", "PUT /actions"]);
        } else {
            assert!(requests.contains(&"PUT /machine-config".into()));
            assert!(requests.contains(&"PUT /boot-source".into()));
            assert!(requests.contains(&"PUT /drives/rootfs".into()));
        }
        server.abort();
    }
}
