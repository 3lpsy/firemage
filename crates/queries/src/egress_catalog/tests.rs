use super::*;
use firemage_orm::{egress_policies, upstream_proxies, vms};
use firemage_wire::{EgressPolicyInput, EgressPolicyUpdate, UpstreamProxyInput};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::json;

async fn fixture() -> (crate::DatabaseConnection, String, String) {
    let db = crate::connect("sqlite::memory:").await.unwrap();
    let owner = crate::add_user(&db, "owner".into(), None, true, None)
        .await
        .unwrap()
        .id;
    let other = crate::add_user(&db, "other".into(), None, true, None)
        .await
        .unwrap()
        .id;
    (db, owner, other)
}
fn policy(alias: &str, proxy: Option<&str>) -> EgressPolicyInput {
    EgressPolicyInput {
        alias: alias.into(),
        policy: firemage_wire::EgressPolicy::default(),
        upstream_proxy_id: proxy.map(str::to_owned),
    }
}
fn proxy(alias: &str) -> UpstreamProxyInput {
    UpstreamProxyInput {
        alias: alias.into(),
        proxy: firemage_wire::UpstreamProxy {
            url: "http://proxy.example:3128".into(),
            username: None,
            password: None,
            ca_pem: None,
        },
        ca_secret: None,
    }
}
async fn vm(
    db: &crate::DatabaseConnection,
    owner: &str,
    name: &str,
    policy: Option<&str>,
) -> vms::Model {
    crate::insert_vm(
        db,
        owner,
        name,
        json!({"name":name,"egress_policy":policy}).to_string(),
        format!("/tmp/{name}.sock"),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn aliases_are_unique_per_owner_and_revisions_reject_lost_updates() {
    let (db, owner, other) = fixture().await;
    let input = policy("review", None);
    let row = insert_egress_policy(&db, &owner, &input).await.unwrap();
    assert!(insert_egress_policy(&db, &owner, &input).await.is_err());
    insert_egress_policy(&db, &other, &input).await.unwrap();
    assert_eq!(egress_policies(&db, Some(&owner)).await.unwrap().len(), 1);
    assert_eq!(egress_policies(&db, None).await.unwrap().len(), 2);
    assert!(egress_policy(&db, &other, &row.id).await.is_err());
    let update = EgressPolicyUpdate {
        revision: 1,
        input: policy("changed", None),
    };
    let updated = update_egress_policy(&db, &owner, &row.id, &update)
        .await
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(
        update_egress_policy(&db, &owner, &row.id, &update)
            .await
            .unwrap_err()
            .is::<CatalogRevisionConflict>()
    );
    assert_eq!(
        egress_policy(&db, &owner, &row.id).await.unwrap().alias,
        "changed"
    );
}

#[tokio::test]
async fn proxy_references_and_secrets_must_belong_to_the_parent_owner() {
    let (db, owner, other) = fixture().await;
    let row = insert_upstream_proxy(&db, &other, &proxy("upstream"))
        .await
        .unwrap();
    assert!(
        insert_egress_policy(&db, &owner, &policy("review", Some(&row.id)))
            .await
            .is_err()
    );
    let mut input = proxy("authenticated");
    input.proxy.username = Some(firemage_wire::ValueSource::Literal("review".into()));
    input.proxy.password = Some(firemage_wire::ValueSource::Secret {
        secret: "proxy-password".into(),
        prefix: String::new(),
    });
    crate::put_secret(&db, &other, "proxy-password", "ciphertext".into())
        .await
        .unwrap();
    assert!(insert_upstream_proxy(&db, &owner, &input).await.is_err());
    crate::put_secret(&db, &owner, "proxy-password", "ciphertext".into())
        .await
        .unwrap();
    let created = insert_upstream_proxy(&db, &owner, &input).await.unwrap();
    assert!(created.spec.contains("proxy-password"));
    assert!(!created.spec.contains("ciphertext"));
    input.ca_secret = Some("missing-ca".into());
    assert!(insert_upstream_proxy(&db, &owner, &input).await.is_err());
}

#[tokio::test]
async fn references_protect_every_vm_state_and_database_foreign_keys() {
    let (db, owner, _) = fixture().await;
    let upstream = insert_upstream_proxy(&db, &owner, &proxy("proxy"))
        .await
        .unwrap();
    let policy = insert_egress_policy(&db, &owner, &policy("review", Some(&upstream.id)))
        .await
        .unwrap();
    let mut machine = vm(&db, &owner, "review", Some(&policy.id)).await;
    for state in [
        "defined", "ready", "starting", "running", "paused", "stopped", "failed", "unknown",
    ] {
        machine = crate::set_vm_state(&db, machine, state, None, None)
            .await
            .unwrap();
        assert!(
            delete_egress_policy(&db, &owner, &policy.id)
                .await
                .unwrap_err()
                .is::<CatalogInUse>()
        );
    }
    assert!(
        delete_upstream_proxy(&db, &owner, &upstream.id)
            .await
            .unwrap_err()
            .is::<CatalogInUse>()
    );
    assert!(
        egress_policies::Entity::delete_by_id(&policy.id)
            .exec(&db)
            .await
            .is_err()
    );
    assert!(
        upstream_proxies::Entity::delete_by_id(&upstream.id)
            .exec(&db)
            .await
            .is_err()
    );
    assert_eq!(
        proxy_vms(&db, &owner, &upstream.id).await.unwrap()[0].id,
        machine.id
    );
    crate::delete_vm(&db, &owner, &machine.id).await.unwrap();
    assert!(
        policy_vms(&db, &owner, &policy.id)
            .await
            .unwrap()
            .is_empty()
    );
    delete_egress_policy(&db, &owner, &policy.id).await.unwrap();
    delete_upstream_proxy(&db, &owner, &upstream.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn failed_binding_rolls_back_vm_spec_and_success_preserves_runtime_state() {
    let (db, owner, other) = fixture().await;
    let own = insert_egress_policy(&db, &owner, &policy("own", None))
        .await
        .unwrap();
    let foreign = insert_egress_policy(&db, &other, &policy("foreign", None))
        .await
        .unwrap();
    let machine = vm(&db, &owner, "review", Some(&own.id)).await;
    let machine = crate::set_vm_state(
        &db,
        machine,
        "running",
        Some("pending change".into()),
        Some(42),
    )
    .await
    .unwrap();
    assert!(
        set_vm_egress_policy(&db, &owner, &machine.id, Some(&foreign.id), 4444)
            .await
            .is_err()
    );
    assert_eq!(
        crate::vm(&db, &owner, &machine.id).await.unwrap().spec,
        machine.spec
    );
    assert_eq!(policy_vms(&db, &owner, &own.id).await.unwrap().len(), 1);
    let updated = set_vm_egress_policy(&db, &owner, &machine.id, None, 4444)
        .await
        .unwrap();
    assert_eq!(updated.state, "running");
    assert_eq!(updated.pid, Some(42));
    assert_eq!(updated.error, machine.error);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&updated.spec).unwrap()["egress_http_port"],
        4444
    );
    assert!(policy_vms(&db, &owner, &own.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn create_and_edit_keep_bindings_atomic() {
    let (db, owner, other) = fixture().await;
    let own = insert_egress_policy(&db, &owner, &policy("own", None))
        .await
        .unwrap();
    let foreign = insert_egress_policy(&db, &other, &policy("foreign", None))
        .await
        .unwrap();
    assert!(
        crate::insert_vm(
            &db,
            &owner,
            "bad",
            json!({"egress_policy":foreign.id}).to_string(),
            "/tmp/bad".into()
        )
        .await
        .is_err()
    );
    assert!(crate::vms(&db, Some(&owner)).await.unwrap().is_empty());
    let machine = vm(&db, &owner, "review", None).await;
    let updated = crate::update_vm_spec(
        &db,
        machine.clone(),
        "edited".into(),
        json!({"egress_policy":own.id}).to_string(),
    )
    .await
    .unwrap();
    assert_eq!(
        policy_vms(&db, &owner, &own.id).await.unwrap()[0].name,
        "edited"
    );
    assert!(
        crate::update_vm_spec(
            &db,
            updated.clone(),
            "failed".into(),
            json!({"egress_policy":foreign.id}).to_string()
        )
        .await
        .is_err()
    );
    assert_eq!(
        crate::vm(&db, &owner, &machine.id).await.unwrap().spec,
        updated.spec
    );
    let mut active: vms::ActiveModel = updated.into();
    active.state = Set("stopped".into());
    active.update(&db).await.unwrap();
}
