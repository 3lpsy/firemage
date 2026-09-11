use crate::mutation::{Change, apply, validate_rows};
use serde_json::json;

#[test]
fn additions_preserve_existing_environment_and_vm_configuration() {
    let spec = json!({"environment":{"TOKEN":{"secret":"service-key"}},"vcpus":4,"metadata":{"owner":"review"}});
    let added = validate_rows(&[
        ("REGION".into(), json!("west")),
        ("EMPTY".into(), json!("")),
    ])
    .unwrap();
    let saved = apply(&spec, Change::Add(added)).unwrap();
    assert_eq!(
        saved["environment"],
        json!({"TOKEN":{"secret":"service-key"},"REGION":"west","EMPTY":""})
    );
    assert_eq!(saved["vcpus"], 4);
    assert_eq!(saved["metadata"], spec["metadata"]);
    assert!(spec["environment"]["REGION"].is_null());
}

#[test]
fn edit_and_delete_touch_only_selected_variable() {
    let spec = json!({"environment":{"REGION":"west","TOKEN":{"secret":"service-key"},"NEW":"concurrent"},"memory_mib":512});
    let saved = apply(
        &spec,
        Change::Edit {
            name: "REGION".into(),
            previous: json!("west"),
            replacement: ("LOCATION".into(), json!("east")),
        },
    )
    .unwrap();
    assert_eq!(
        saved["environment"],
        json!({"LOCATION":"east","TOKEN":{"secret":"service-key"},"NEW":"concurrent"})
    );
    let deleted = apply(
        &saved,
        Change::Delete {
            name: "LOCATION".into(),
            previous: json!("east"),
        },
    )
    .unwrap();
    assert_eq!(
        deleted["environment"],
        json!({"TOKEN":{"secret":"service-key"},"NEW":"concurrent"})
    );
    assert_eq!(deleted["memory_mib"], 512);
}

#[test]
fn stale_edits_and_deletes_do_not_overwrite_another_change() {
    for spec in [
        json!({"environment":{"REGION":"changed"}}),
        json!({"environment":{}}),
    ] {
        assert!(
            apply(
                &spec,
                Change::Edit {
                    name: "REGION".into(),
                    previous: json!("west"),
                    replacement: ("REGION".into(), json!("east"))
                }
            )
            .is_err()
        );
        assert!(
            apply(
                &spec,
                Change::Delete {
                    name: "REGION".into(),
                    previous: json!("west")
                }
            )
            .is_err()
        );
    }
}

#[test]
fn new_names_cannot_replace_existing_variables() {
    let spec = json!({"environment":{"REGION":"west","TOKEN":{"secret":"service-key"}}});
    let values = validate_rows(&[("REGION".into(), json!("east"))]).unwrap();
    assert!(apply(&spec, Change::Add(values)).is_err());
    assert!(
        apply(
            &spec,
            Change::Edit {
                name: "REGION".into(),
                previous: json!("west"),
                replacement: ("TOKEN".into(), json!("replacement"))
            }
        )
        .is_err()
    );
    assert!(apply(&spec, Change::Add(Default::default())).is_err());
}

#[test]
fn row_validation_rejects_invalid_names_duplicates_and_missing_secrets() {
    for name in ["", "2BAD", "A-B", "WITH SPACE"] {
        assert!(validate_rows(&[(name.into(), json!("value"))]).is_err());
    }
    assert!(
        validate_rows(&[("NAME".into(), json!("one")), ("NAME".into(), json!("two"))]).is_err()
    );
    assert!(validate_rows(&[("TOKEN".into(), json!({"secret":""}))]).is_err());
    assert!(validate_rows(&[("TOKEN".into(), json!(false))]).is_err());
    assert!(validate_rows(&[("_VALID_2".into(), json!({"secret":"service-key"}))]).is_ok());
}
