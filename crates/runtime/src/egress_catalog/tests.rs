use crate::Runtime;
use firemage_wire::{
    EgressPolicyInput, EgressPolicyUpdate, UpstreamProxyInput, VmConfigImport, VmSpec,
};
use serde_json::json;

async fn fixture() -> (tempfile::TempDir, Runtime, String) {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "operator".into(), None, true, None)
        .await
        .unwrap()
        .id;
    firemage_queries::insert_network(&db, &owner, "private", json!({"name":"private","subnet":"10.42.0.0/24","gateway":"10.42.0.1","policy":{"mode":"firemage-only"}}).to_string()).await.unwrap();
    let runtime = Runtime::new(
        db,
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    (directory, runtime, owner)
}
fn spec(name: &str, address: &str) -> VmSpec {
    serde_json::from_value(json!({"name":name,"network":{"network":"private","address":address,"mac":"02:00:00:00:00:02"}})).unwrap()
}
fn policy(alias: &str) -> EgressPolicyInput {
    serde_json::from_value(
        json!({"alias":alias,"policy":{"inherit_upstream":false,"http":{"port":3128,"rules":[]}}}),
    )
    .unwrap()
}

#[tokio::test]
async fn shared_policy_references_survive_rename_and_export_by_alias() {
    let (_dir, runtime, owner) = fixture().await;
    let stored = runtime
        .create_egress_policy(&owner, policy("review"))
        .await
        .unwrap();
    let mut first = spec("first", "10.42.0.2");
    first.egress_policy = Some(stored.id.clone());
    let first = runtime.define(&owner, first).await.unwrap();
    let mut second = spec("second", "10.42.0.3");
    second.network.as_mut().unwrap().mac = "02:00:00:00:00:03".into();
    second.egress_policy = Some(stored.id.clone());
    let second = runtime.define(&owner, second).await.unwrap();
    assert!(
        runtime
            .delete_egress_policy(&owner, &stored.id)
            .await
            .is_err()
    );
    let renamed = runtime
        .update_egress_policy(
            &owner,
            &stored.id,
            EgressPolicyUpdate {
                revision: stored.revision as u64,
                input: policy("renamed"),
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.id, stored.id);
    let exported = runtime.export_vm_config(&owner, &first.id).await.unwrap();
    assert!(exported.contains("egress_policy_alias = \"renamed\""));
    assert!(!exported.contains(&stored.id));
    let (imported, _) = runtime
        .resolve_vm_config(
            &owner,
            &VmConfigImport {
                toml: exported,
                name: Some("imported".into()),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(imported.egress_policy.as_deref(), Some(stored.id.as_str()));
    runtime
        .assign_egress_policy(&owner, &first.id, None)
        .await
        .unwrap();
    assert!(
        runtime
            .delete_egress_policy(&owner, &stored.id)
            .await
            .is_err()
    );
    runtime
        .assign_egress_policy(&owner, &second.id, None)
        .await
        .unwrap();
    runtime
        .delete_egress_policy(&owner, &stored.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn migration_preserves_independent_inline_policies_and_live_state() {
    let (_dir, runtime, owner) = fixture().await;
    let mut ids = Vec::new();
    for (name, address) in [("one", "10.42.0.2"), ("two", "10.42.0.3")] {
        let mut vm = spec(name, address);
        let mut input = policy("unused").policy;
        input.http.as_mut().unwrap().port = 9128;
        vm.egress = Some(input);
        let row = firemage_queries::insert_vm(
            &runtime.db,
            &owner,
            name,
            serde_json::to_string(&vm).unwrap(),
            format!("/tmp/{name}.sock"),
        )
        .await
        .unwrap();
        let row = firemage_queries::set_vm_state(
            &runtime.db,
            row,
            "running",
            Some("existing message".into()),
            None,
        )
        .await
        .unwrap();
        ids.push(row.id);
    }
    runtime.migrate_egress_catalog().await.unwrap();
    runtime.migrate_egress_catalog().await.unwrap();
    assert_eq!(
        firemage_queries::egress_policies(&runtime.db, Some(&owner))
            .await
            .unwrap()
            .len(),
        2
    );
    let mut policies = Vec::new();
    for id in ids {
        let row = firemage_queries::vm(&runtime.db, &owner, &id)
            .await
            .unwrap();
        assert_eq!(row.state, "running");
        assert_eq!(row.error.as_deref(), Some("existing message"));
        let spec: VmSpec = serde_json::from_str(&row.spec).unwrap();
        assert!(spec.egress.is_none());
        assert_eq!(spec.egress_http_port, 9128);
        policies.push(spec.egress_policy.unwrap());
    }
    assert_ne!(policies[0], policies[1]);
}

#[tokio::test]
async fn stopped_vm_custom_port_conflict_rejects_shared_policy_update() {
    let (_dir, runtime, owner) = fixture().await;
    let stored = runtime
        .create_egress_policy(&owner, policy("review"))
        .await
        .unwrap();
    let mut vm = spec("review", "10.42.0.2");
    vm.egress_policy = Some(stored.id.clone());
    vm.egress_http_port = 9128;
    runtime.define(&owner, vm).await.unwrap();
    let mut input = policy("review");
    input.policy.tunnels.push(serde_json::from_value(json!({"name":"database","listen_port":9128,"target_host":"database.example.com","target_port":5432})).unwrap());
    assert!(
        runtime
            .update_egress_policy(
                &owner,
                &stored.id,
                EgressPolicyUpdate {
                    revision: stored.revision as u64,
                    input
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        firemage_queries::egress_policy(&runtime.db, &owner, &stored.id)
            .await
            .unwrap()
            .revision,
        stored.revision
    );
}

#[tokio::test]
async fn proxy_ca_secret_is_checked_before_catalog_mutation() {
    let (_dir, runtime, owner) = fixture().await;
    runtime
        .secrets()
        .await
        .unwrap()
        .put(
            &owner,
            "invalid-ca",
            "-----BEGIN CERTIFICATE-----\ninvalid\n-----END CERTIFICATE-----",
        )
        .await
        .unwrap();
    let input: UpstreamProxyInput = serde_json::from_value(json!({"alias":"proxy","proxy":{"url":"https://proxy.example.com:3129"},"ca_secret":"invalid-ca"})).unwrap();
    assert!(runtime.create_upstream_proxy(&owner, input).await.is_err());
    assert!(
        firemage_queries::upstream_proxies(&runtime.db, Some(&owner))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn restricted_guest_bootstrap_exists_without_initial_policy() {
    let (_dir, runtime, owner) = fixture().await;
    let files = runtime
        .seed_files(&owner, &spec("review", "10.42.0.2"))
        .await
        .unwrap();
    assert!(files.iter().any(|file| file.path == "firemage/ca.pem"));
    let environment = files
        .iter()
        .find(|file| file.path == "firemage/environment.sh")
        .unwrap();
    assert!(environment.content.contains("http://10.42.0.1:3128"));
}

#[tokio::test]
async fn live_update_rechecks_a_vm_that_changed_while_waiting_for_its_lock() {
    let (_dir, runtime, owner) = fixture().await;
    let vm = runtime
        .define(&owner, spec("review", "10.42.0.2"))
        .await
        .unwrap();
    let row = firemage_queries::vm(&runtime.db, &owner, &vm.id)
        .await
        .unwrap();
    let stale = firemage_queries::set_vm_state(&runtime.db, row, "failed", None, None)
        .await
        .unwrap();
    firemage_queries::set_vm_state(&runtime.db, stale.clone(), "running", None, None)
        .await
        .unwrap();
    let _guard = runtime.lock(&vm.id).await;
    let result = runtime.live_change(stale, None).await;
    assert!(
        result.is_err(),
        "a previously failed VM must not skip live validation after restarting"
    );
    assert_eq!(
        firemage_queries::vm(&runtime.db, &owner, &vm.id)
            .await
            .unwrap()
            .state,
        "unknown"
    );
}

#[tokio::test]
async fn unchanged_policy_retries_failed_recovery_instead_of_reporting_success() {
    let (_dir, runtime, owner) = fixture().await;
    let stored = runtime
        .create_egress_policy(&owner, policy("review"))
        .await
        .unwrap();
    let mut definition = spec("review", "10.42.0.2");
    definition.egress_policy = Some(stored.id.clone());
    let vm = runtime.define(&owner, definition).await.unwrap();
    let row = firemage_queries::vm(&runtime.db, &owner, &vm.id)
        .await
        .unwrap();
    firemage_queries::set_vm_state(&runtime.db, row, "running", None, None)
        .await
        .unwrap();
    runtime.recover_egress().await.unwrap();
    // Older servers recorded this error without a recovery marker.
    std::fs::remove_file(runtime.egress_pending(&vm.id)).unwrap();
    let result = runtime
        .update_egress_policy(
            &owner,
            &stored.id,
            EgressPolicyUpdate {
                revision: stored.revision as u64,
                input: policy("review"),
            },
        )
        .await;
    assert!(
        result.is_err(),
        "saving the unchanged policy must verify the blocked VM before reporting success"
    );
    assert_eq!(
        firemage_queries::egress_policy(&runtime.db, &owner, &stored.id)
            .await
            .unwrap()
            .revision,
        stored.revision
    );
}

#[tokio::test]
async fn rejected_vm_definition_does_not_create_inline_catalog_resources() {
    let (_dir, runtime, owner) = fixture().await;
    runtime
        .define(&owner, spec("first", "10.42.0.2"))
        .await
        .unwrap();
    let mut duplicate = spec("duplicate", "10.42.0.2");
    duplicate.egress = Some(policy("unused").policy);
    assert!(runtime.define(&owner, duplicate).await.is_err());
    assert!(
        firemage_queries::egress_policies(&runtime.db, Some(&owner))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn deleting_vm_removes_its_pending_egress_recovery() {
    let (_dir, runtime, owner) = fixture().await;
    let vm = runtime
        .define(&owner, spec("review", "10.42.0.2"))
        .await
        .unwrap();
    runtime.mark_egress_pending(&vm.id).unwrap();
    runtime.delete(&owner, &vm.id).await.unwrap();
    assert!(!runtime.egress_pending(&vm.id).exists());
}
