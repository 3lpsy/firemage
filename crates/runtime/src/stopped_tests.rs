use crate::Runtime;
use serde_json::json;

#[tokio::test]
async fn failed_vm_with_live_process_or_socket_keeps_its_disks() {
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
    for with_pid in [true, false] {
        let vm = runtime
            .define(
                &user.id,
                serde_json::from_value(json!({"name":format!("failed-{with_pid}")})).unwrap(),
            )
            .await
            .unwrap();
        let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
        std::fs::create_dir_all(runtime.directory(&vm.id)).unwrap();
        let disk = runtime.directory(&vm.id).join("rootfs.ext4");
        std::fs::write(&disk, b"preserve").unwrap();
        let mut listener = None;
        let pid = if with_pid {
            let pid = std::process::id() as i32;
            firemage_queries::set_process(&db, &vm.id, pid, crate::process::identity(pid).unwrap())
                .await
                .unwrap();
            Some(pid)
        } else {
            std::fs::create_dir_all(std::path::Path::new(&row.socket).parent().unwrap()).unwrap();
            listener = Some(tokio::net::UnixListener::bind(&row.socket).unwrap());
            None
        };
        let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
        firemage_queries::set_vm_state(&db, row, "failed", None, pid)
            .await
            .unwrap();
        assert!(runtime.delete(&user.id, &vm.id).await.is_err());
        assert_eq!(std::fs::read(&disk).unwrap(), b"preserve");
        assert!(firemage_queries::vm(&db, &user.id, &vm.id).await.is_ok());
        drop(listener);
    }
}
