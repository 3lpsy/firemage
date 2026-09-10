#[tokio::test]
async fn unresolved_registry_settings_never_start_an_anonymous_pull() {
    let directory = tempfile::tempdir().unwrap();
    for registry in [
        serde_json::json!({"auth":{"kind":"bearer","token_secret":"registry-token"}}),
        serde_json::json!({"ca_secret":"registry-ca"}),
        serde_json::json!({"token_realm":"https://auth.example/token"}),
    ] {
        let asset = serde_json::from_value(serde_json::json!({
            "kind":"oci", "image":format!("registry.invalid/image@sha256:{}", "a".repeat(64)), "registry":registry
        })).unwrap();
        let error = firemage_assets::materialize(&asset, &directory.path().join("rootfs.ext4"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("must be resolved"));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
