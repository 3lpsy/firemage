use firemage_config::{Config, ConfigEdit, ManagedConfig, Server};

#[test]
fn asset_limit_defaults_validation_and_file_configuration() {
    let defaults = Server::default();
    assert_eq!(defaults.asset_max_bytes(), 1024 * 1024 * 1024);
    assert_eq!(
        defaults.seed_max_bytes(),
        defaults.asset_max_bytes() + 128 * 1024 * 1024
    );
    defaults.validate().unwrap();
    let file: Config = toml::from_str("[server]\nasset_max_bytes=4096").unwrap();
    let resolved = Server::default().merge(file.server);
    assert_eq!(resolved.asset_max_bytes(), 4096);
    resolved.validate().unwrap();
    for maximum in [0, u64::MAX, isize::MAX as u64, 32 * 1024 * 1024 * 1024] {
        assert!(
            Server {
                asset_max_bytes: Some(maximum),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    Server {
        asset_max_bytes: Some(24 * 1024 * 1024 * 1024),
        ..Default::default()
    }
    .validate()
    .unwrap();
    assert!(toml::from_str::<Config>("[server]\nasset_max_bytes=-1").is_err());
}

#[test]
fn asset_limit_is_host_managed_and_reports_pending_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::write(&path, "[server]\nasset_max_bytes=4096").unwrap();
    let startup = firemage_config::read(&path, true).unwrap().server;
    let managed = ManagedConfig::new(Some(path.clone()), Server::default(), startup);
    let view = managed.view().unwrap();
    assert!(view.host_only.contains(&"asset_max_bytes".into()));
    assert_eq!(view.effective["asset_max_bytes"], 4096);
    assert!(
        managed
            .edit(
                ConfigEdit {
                    toml: view.toml.replace("4096", "8192"),
                    revision: view.revision,
                },
                true
            )
            .unwrap_err()
            .to_string()
            .contains("host-managed")
    );
    std::fs::write(path, "[server]\nasset_max_bytes=8192").unwrap();
    let view = managed.view().unwrap();
    assert_eq!(view.effective["asset_max_bytes"], 4096);
    assert_eq!(view.effective_after_restart["asset_max_bytes"], 8192);
    assert!(view.restart_required.contains(&"asset_max_bytes".into()));
}

#[test]
fn snapshot_storage_defaults_and_host_limits_are_configurable() {
    let defaults = Server::default();
    assert_eq!(
        defaults.snapshot_dir(),
        defaults.data_dir().join("snapshots")
    );
    assert_eq!(defaults.snapshot_max_bytes(), 64 * 1024 * 1024 * 1024);
    let config: Config =
        toml::from_str("[server]\nsnapshot_dir='/srv/snapshots'\nsnapshot_max_bytes=1073741824")
            .unwrap();
    let server = defaults.merge(config.server);
    server.validate().unwrap();
    assert_eq!(
        server.snapshot_dir(),
        std::path::PathBuf::from("/srv/snapshots")
    );
    assert_eq!(server.snapshot_max_bytes(), 1073741824);
    let view = ManagedConfig::new(None, Server::default(), server)
        .view()
        .unwrap();
    assert!(view.host_only.contains(&"snapshot_dir".into()));
    assert!(view.host_only.contains(&"snapshot_max_bytes".into()));
    for limit in [0, u64::MAX] {
        assert!(
            Server {
                snapshot_max_bytes: Some(limit),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Server {
            snapshot_dir: Some("relative".into()),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
