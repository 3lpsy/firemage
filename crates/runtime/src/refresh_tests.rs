use crate::Runtime;
use axum::{Json, Router, extract::State, routing::get};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn jail_verification_racing_exit_stops_but_live_unverified_process_stays_unknown() {
    for exits in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
        let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
            .await
            .unwrap();
        let runtime = Runtime::new(
            db.clone(),
            firemage_config::Server {
                data_dir: Some(directory.path().into()),
                ..Default::default()
            },
        );
        let vm = runtime
            .define(
                &user.id,
                serde_json::from_value(json!({"name":"exit-race"})).unwrap(),
            )
            .await
            .unwrap();
        let child = tokio::process::Command::new("sleep")
            .arg("60")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap() as i32;
        firemage_queries::set_process(&db, &vm.id, pid, crate::process::identity(pid).unwrap())
            .await
            .unwrap();
        let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
        let row = firemage_queries::set_vm_state(&db, row, "running", None, Some(pid))
            .await
            .unwrap();
        std::fs::create_dir_all(std::path::Path::new(&row.socket).parent().unwrap()).unwrap();
        let listener = tokio::net::UnixListener::bind(&row.socket).unwrap();
        let child = Arc::new(Mutex::new(child));
        let app = Router::new()
            .route(
                "/",
                get(
                    move |State(child): State<Arc<Mutex<tokio::process::Child>>>| async move {
                        // Deliver the stale API response only after the recorded process has exited.
                        if exits {
                            child.lock().await.kill().await.unwrap();
                        }
                        Json(json!({"state":"Running"}))
                    },
                ),
            )
            .with_state(child.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = runtime.refresh(row).await.unwrap();
        assert_eq!(result.state, if exits { "stopped" } else { "unknown" });
        assert_eq!(result.error.is_some(), !exits);
        if !exits {
            child.lock().await.kill().await.unwrap();
        }
        server.abort();
    }
}
