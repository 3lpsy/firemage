use firemage_runtime::Runtime;
use firemage_wire::{IsolationMode, VmAction, VmSpec};
use serde_json::json;

#[tokio::test]
async fn defaults_fail_closed_and_elevated_modes_require_host_policy() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let config = firemage_config::Server {
        data_dir: Some(dir.path().into()),
        ..Default::default()
    };
    let runtime = Runtime::new(db.clone(), config.clone());
    let mut spec: VmSpec = serde_json::from_value(json!({"name":"locked"})).unwrap();
    assert_eq!(spec.security.mode, IsolationMode::Jailed);
    let vm = runtime.define(&user.id, spec.clone()).await.unwrap();
    assert!(
        runtime
            .action(&user.id, &vm.id, VmAction::Launch)
            .await
            .unwrap_err()
            .to_string()
            .contains("manual launch requires trusted mode")
    );
    spec.security.mode = IsolationMode::Trusted;
    assert!(
        runtime
            .define(&user.id, spec.clone())
            .await
            .unwrap_err()
            .to_string()
            .contains("disabled by host policy")
    );
    let enabled = Runtime::new(
        db.clone(),
        firemage_config::Server {
            allow_trusted_vms: Some(true),
            ..config
        },
    );
    let vm = enabled.define(&user.id, spec.clone()).await.unwrap();
    spec.security.mode = IsolationMode::Jailed;
    assert!(
        enabled
            .update(&user.id, &vm.id, spec)
            .await
            .unwrap_err()
            .to_string()
            .contains("isolation mode cannot be changed")
    );
}

#[tokio::test]
async fn jailed_raw_api_cannot_add_host_resources_or_change_limits() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(dir.path().into()),
            ..Default::default()
        },
    );
    let spec: VmSpec = serde_json::from_value(json!({"name":"locked"})).unwrap();
    let vm = runtime.define(&user.id, spec).await.unwrap();
    let row = firemage_queries::vm(&db, &user.id, &vm.id).await.unwrap();
    for path in [
        "/drives/escape",
        "/boot-source",
        "/logger",
        "/metrics",
        "/vsock",
        "/network-interfaces/eth0",
        "/snapshot/load",
        "/snapshot/create",
        "/machine-config",
    ] {
        let input = firemage_wire::RawRequest {
            method: "PUT".into(),
            path: path.into(),
            body: Some(json!({"path_on_host":"/etc/shadow"})),
        };
        assert!(
            runtime.ensure_raw_request(&row, &input).await.is_err(),
            "{path}"
        );
    }
    let input = firemage_wire::RawRequest {
        method: "GET".into(),
        path: "/machine-config".into(),
        body: None,
    };
    runtime.ensure_raw_request(&row, &input).await.unwrap();
}

#[test]
fn external_socket_is_an_explicit_choice_and_limits_are_validated() {
    let mut spec: VmSpec =
        serde_json::from_value(json!({"name":"external", "socket":"/run/firecracker.sock"}))
            .unwrap();
    assert!(spec.validate().is_err());
    spec.security.mode = IsolationMode::External;
    spec.validate().unwrap();
    spec.security.pids_max = 1;
    assert!(spec.validate().is_err());
    spec.security.pids_max = 128;
    spec.security.cpu_percent = Some(101);
    assert!(spec.validate().is_err());
}

#[tokio::test]
async fn unrestricted_raw_api_requires_both_host_permissions_and_never_applies_to_jails() {
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let user = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap();
    for mode in ["jailed", "trusted", "external"] {
        let row = firemage_queries::insert_vm(&db, &user.id, mode,
            json!({"name":mode, "security":{"mode":mode}, "socket": if mode == "external" { Some("/run/vmm.sock") } else { None }}).to_string(),
            format!("/run/{mode}.sock")).await.unwrap();
        for enabled in [false, true] {
            for mode_enabled in [false, true] {
                let runtime = Runtime::new(
                    db.clone(),
                    firemage_config::Server {
                        allow_trusted_vms: Some(mode_enabled),
                        allow_external_vms: Some(mode_enabled),
                        allow_unrestricted_raw_api: Some(enabled),
                        ..Default::default()
                    },
                );
                for path in ["/drives/escape", "/boot-source", "/network-interfaces/eth1"] {
                    let input = firemage_wire::RawRequest {
                        method: "PUT".into(),
                        path: path.into(),
                        body: Some(json!({"path_on_host":"/etc/shadow"})),
                    };
                    assert_eq!(
                        runtime.ensure_raw_request(&row, &input).await.is_ok(),
                        enabled && mode_enabled && mode != "jailed",
                        "{mode}: host flag={enabled}, mode allowed={mode_enabled}, {path}"
                    );
                }
                for (method, path) in [("GET", "/machine-config"), ("PATCH", "/vm")] {
                    let input = firemage_wire::RawRequest {
                        method: method.into(),
                        path: path.into(),
                        body: None,
                    };
                    runtime.ensure_raw_request(&row, &input).await.unwrap();
                }
            }
        }
    }
}
