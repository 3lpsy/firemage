use firemage_runtime::Runtime;
use firemage_wire::{VmConfigDocument, VmConfigImport, VmSpec};
use serde_json::json;

#[tokio::test]
async fn portable_networks_use_current_names_and_resolve_back_to_owned_uuid() {
    let directory = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap();
    let other = firemage_queries::add_user(&db, "other".into(), None, true, None)
        .await
        .unwrap();
    let runtime = Runtime::new(
        db.clone(),
        firemage_config::Server {
            data_dir: Some(directory.path().into()),
            ..Default::default()
        },
    );
    let mut network = json!({"name":"original","subnet":"10.88.1.0/24","gateway":"10.88.1.1","policy":{"mode":"isolated"}});
    firemage_queries::insert_network(&db, &owner.id, "original", network.to_string())
        .await
        .unwrap();
    let row = firemage_queries::network(&db, &owner.id, "original")
        .await
        .unwrap();
    let spec: VmSpec = serde_json::from_value(json!({"name":"review","network":{
        "network":"original","address":"10.88.1.10","mac":"02:00:00:00:00:10"
    }}))
    .unwrap();
    let vm = runtime.define(&owner.id, spec).await.unwrap();
    assert_eq!(vm.spec.network.as_ref().unwrap().network, row.id);
    let before: VmConfigDocument =
        toml::from_str(&runtime.export_vm_config(&owner.id, &vm.id).await.unwrap()).unwrap();
    assert_eq!(before.vm.network.unwrap().network, "original");
    network["name"] = json!("renamed");
    firemage_queries::update_network(&db, row.clone(), network.to_string())
        .await
        .unwrap();
    let encoded = runtime.export_vm_config(&owner.id, &vm.id).await.unwrap();
    assert!(!encoded.contains(&row.id));
    let document: VmConfigDocument = toml::from_str(&encoded).unwrap();
    assert_eq!(document.vm.network.as_ref().unwrap().network, "renamed");
    let input = VmConfigImport {
        toml: encoded,
        name: Some("copy".into()),
    };
    let (resolved, preview) = runtime
        .resolve_vm_config(&owner.id, &input, None)
        .await
        .unwrap();
    let attached = resolved.network.as_ref().unwrap();
    assert_eq!(attached.network, row.id);
    assert_ne!(attached.address, vm.spec.network.as_ref().unwrap().address);
    assert!(
        preview
            .references
            .iter()
            .any(|reference| reference.kind == "Network" && reference.alias == "renamed")
    );
    assert!(
        runtime
            .resolve_vm_config(&other.id, &input, None)
            .await
            .is_err()
    );
    let mut raw_id = document;
    raw_id.vm.network.as_mut().unwrap().network = row.id;
    assert!(
        runtime
            .resolve_vm_config(
                &owner.id,
                &VmConfigImport {
                    toml: toml::to_string(&raw_id).unwrap(),
                    name: Some("invalid-id-import".into())
                },
                None
            )
            .await
            .is_err()
    );
    let mut updated = vm.spec;
    updated.network.as_mut().unwrap().network = "renamed".into();
    let updated = runtime.update(&owner.id, &vm.id, updated).await.unwrap();
    assert_eq!(updated.spec.network.unwrap().network, attached.network);
}
