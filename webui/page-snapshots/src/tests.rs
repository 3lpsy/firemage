use serde_json::json;

#[test]
fn restore_choices_disambiguate_aliases_across_owners() {
    let first = json!({"alias":"checkpoint", "id":"01234567-a", "owner_id":"first"});
    let second = json!({"alias":"checkpoint", "id":"01234567-b", "owner_id":"second"});
    let chosen = crate::model::snapshot_choice(&second);
    assert_ne!(crate::model::snapshot_choice(&first), chosen);
    let choices = [first, second];
    let selected = choices
        .iter()
        .find(|snapshot| crate::model::snapshot_choice(snapshot) == chosen)
        .unwrap();
    assert_eq!(selected["owner_id"], "second");
}

#[test]
fn capture_and_restore_require_distinct_lifecycle_states() {
    let mut vm = json!({"state":"paused", "spec":{"vcpus":2,"memory_mib":4096,"security":{"mode":"jailed"}}});
    let snapshot = json!({"vcpus":2,"memory_mib":4096});
    assert!(crate::model::is_capture_target(&vm));
    assert!(!crate::model::is_restore_target(&vm, &snapshot));
    for state in ["defined", "stopped", "failed"] {
        vm["state"] = json!(state);
        assert!(crate::model::is_restore_target(&vm, &snapshot));
        assert!(!crate::model::is_capture_target(&vm));
    }
    vm["spec"]["memory_mib"] = json!(2048);
    assert!(!crate::model::is_restore_target(&vm, &snapshot));
    vm["spec"]["memory_mib"] = json!(4096);
    vm["spec"]["security"]["mode"] = json!("trusted");
    assert!(!crate::model::is_restore_target(&vm, &snapshot));
}

#[test]
fn vm_inventory_uses_source_id_and_keeps_uploaded_snapshots_unbound() {
    let vm = json!({"id":"source-id", "spec":{"name":"renamed"}});
    assert!(crate::model::is_source_vm(
        &vm,
        &json!({"source_vm_id":"source-id", "source_vm_name":"original"})
    ));
    assert!(!crate::model::is_source_vm(
        &vm,
        &json!({"source_vm_id":"other-id", "source_vm_name":"renamed"})
    ));
    assert!(!crate::model::is_source_vm(
        &vm,
        &json!({"source_vm_id":null, "source_vm_name":"renamed"})
    ));
}
