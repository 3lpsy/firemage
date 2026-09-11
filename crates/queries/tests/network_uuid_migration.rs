use firemage_migrations::{Migrator, MigratorTrait};
use firemage_orm::{file_assets, networks, vms};
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, Set};
use serde_json::{Value, json};

async fn network(db: &DatabaseConnection, owner: &str, name: &str) -> networks::Model {
    networks::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        name: Set(name.into()),
        spec: Set(json!({"name":name,"subnet":"10.99.1.0/24","gateway":"10.99.1.1","policy":{"mode":"isolated"}}).to_string()),
    }.insert(db).await.unwrap()
}

async fn vm(
    db: &DatabaseConnection,
    owner: &str,
    reference: Option<&str>,
    state: &str,
) -> vms::Model {
    let id = uuid::Uuid::new_v4().to_string();
    let mut spec = json!({"name":"migration-guest", "metadata":{"custom":"preserve"},
        "environment":{"VALUE":"unchanged"}, "future-field":{"nested":[1,2,3]}});
    if let Some(reference) = reference {
        spec["network"] =
            json!({"network":reference,"address":"10.99.1.10","mac":"02:00:00:00:00:10"});
    }
    vms::ActiveModel {
        id: Set(id.clone()),
        owner_id: Set(owner.into()),
        name: Set("migration-guest".into()),
        spec: Set(spec.to_string()),
        state: Set(state.into()),
        error: Set(Some("retained diagnostic".into())),
        socket: Set(format!("/tmp/{id}.sock")),
        pid: Set(Some(1234)),
        process_start: Set(Some("5678".into())),
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn upgrade_resolves_owner_names_and_preserves_uuid_references_state_and_metadata() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, Some(14)).await.unwrap();
    let first = firemage_queries::add_user(&db, "first".into(), None, true, None)
        .await
        .unwrap();
    let second = firemage_queries::add_user(&db, "second".into(), None, true, None)
        .await
        .unwrap();
    let first_net = network(&db, &first.id, "first-network").await;
    let second_net = network(&db, &second.id, "second-network").await;
    let asset = file_assets::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(first.id.clone()),
        alias: Set("existing".into()),
        filename: Set("input.txt".into()),
        size_bytes: Set(7),
        sha256: Set("0".repeat(64)),
        created_at: Set(123),
        storage_name: Set(Some("managed-input".into())),
    }
    .insert(&db)
    .await
    .unwrap();
    let rows = [
        (
            vm(&db, &first.id, Some("first-network"), "running").await,
            Some(first_net.id.clone()),
        ),
        (
            vm(&db, &second.id, Some("second-network"), "stopped").await,
            Some(second_net.id.clone()),
        ),
        (
            vm(&db, &first.id, Some(&first_net.id), "paused").await,
            Some(first_net.id.clone()),
        ),
        (vm(&db, &first.id, None, "defined").await, None),
    ];
    Migrator::up(&db, None).await.unwrap();
    for (mut expected, reference) in rows {
        if let Some(reference) = reference {
            let mut spec: Value = serde_json::from_str(&expected.spec).unwrap();
            spec["network"]["network"] = json!(reference);
            expected.spec = serde_json::to_string(&spec).unwrap();
        }
        let actual = vms::Entity::find_by_id(&expected.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
    }
    assert_eq!(
        file_assets::Entity::find_by_id(&asset.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        asset
    );
    assert_eq!(
        networks::Entity::find_by_id(&first_net.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap(),
        first_net
    );
}

#[tokio::test]
async fn unresolved_or_foreign_network_references_abort_without_partial_vm_changes() {
    for foreign_uuid in [false, true] {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, Some(14)).await.unwrap();
        let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
            .await
            .unwrap();
        let other = firemage_queries::add_user(&db, "other".into(), None, true, None)
            .await
            .unwrap();
        let own_net = network(&db, &owner.id, "own-network").await;
        let foreign = network(&db, &other.id, "foreign-only").await;
        let valid = vm(&db, &owner.id, Some(&own_net.name), "running").await;
        let invalid = vm(
            &db,
            &owner.id,
            Some(if foreign_uuid {
                &foreign.id
            } else {
                &foreign.name
            }),
            "stopped",
        )
        .await;
        assert!(Migrator::up(&db, None).await.is_err());
        for expected in [&valid, &invalid] {
            assert_eq!(
                &vms::Entity::find_by_id(&expected.id)
                    .one(&db)
                    .await
                    .unwrap()
                    .unwrap(),
                expected
            );
        }
        let mut repaired: vms::ActiveModel = invalid.into();
        let mut spec: Value = serde_json::from_str(&valid.spec).unwrap();
        spec["name"] = json!("repaired");
        repaired.spec = Set(spec.to_string());
        repaired.update(&db).await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let migrated = vms::Entity::find_by_id(valid.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&migrated.spec).unwrap()["network"]["network"],
            own_net.id
        );
    }
}

#[tokio::test]
async fn ambiguous_references_and_duplicate_names_roll_back_the_upgrade() {
    for ambiguous in [false, true] {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, Some(14)).await.unwrap();
        let owner = firemage_queries::add_user(&db, "owner".into(), None, true, None)
            .await
            .unwrap();
        let first = network(&db, &owner.id, "original").await;
        network(
            &db,
            &owner.id,
            if ambiguous { &first.id } else { &first.name },
        )
        .await;
        let before = vm(
            &db,
            &owner.id,
            Some(if ambiguous { &first.id } else { &first.name }),
            "running",
        )
        .await;
        assert!(Migrator::up(&db, None).await.is_err());
        assert_eq!(
            vms::Entity::find_by_id(&before.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap(),
            before
        );
    }
}
