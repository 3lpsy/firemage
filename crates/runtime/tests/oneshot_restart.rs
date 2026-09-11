use firemage_runtime::Runtime;
use firemage_wire::{VmAction, VmState};
use serde_json::json;

#[tokio::test]
async fn completed_one_shot_starts_again_without_an_intermediate_stop_or_refresh() {
    for stale_state in ["running", "unknown", "stopped"] {
        let directory = tempfile::tempdir().unwrap();
        let kernel_dir = directory.path().join("kernels");
        std::fs::create_dir(&kernel_dir).unwrap();
        std::fs::write(kernel_dir.join("kernel"), b"kernel").unwrap();
        let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
        let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
            .await
            .unwrap();
        let runtime = Runtime::new(
            db.clone(),
            firemage_config::Server {
                data_dir: Some(directory.path().into()),
                kernel_dir: Some(kernel_dir),
                firecracker: Some("/usr/bin/python3".into()),
                firecracker_args: Some(vec![format!(
                    "{}/tests/fixtures/oneshot_vmm.py",
                    env!("CARGO_MANIFEST_DIR")
                )]),
                allow_trusted_vms: Some(true),
                ..Default::default()
            },
        );
        let spec = serde_json::from_value(json!({
            "name":"oneshot", "security":{"mode":"trusted"},
            "kernel":{"kind":"kernel","name":"kernel"},
            "rootfs":{"kind":"oci","image":"example.invalid/runner:stable","size_mib":128}
        }))
        .unwrap();
        let vm = runtime.define(&user.id, spec).await.unwrap();
        std::fs::create_dir_all(runtime.directory(&vm.id)).unwrap();
        std::fs::write(runtime.directory(&vm.id).join("rootfs.ext4"), b"root disk").unwrap();
        for run in 0..2 {
            let started = runtime
                .action(&user.id, &vm.id, VmAction::Start)
                .await
                .unwrap();
            assert_eq!(started.state, VmState::Running);
            let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while std::fs::read_to_string(format!("/proc/{}/stat", row.pid.unwrap()))
                    .is_ok_and(|stat| !stat.rsplit_once(") ").unwrap().1.starts_with("Z "))
                {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            if run == 0 {
                let pid = row.pid;
                firemage_queries::set_vm_state(&db, row, stale_state, None, pid)
                    .await
                    .unwrap();
            }
        }
        assert_eq!(
            runtime
                .action(&user.id, &vm.id, VmAction::Refresh)
                .await
                .unwrap()
                .state,
            VmState::Stopped
        );
    }
}
