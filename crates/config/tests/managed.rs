use firemage_config::{ConfigEdit, ManagedConfig, Server};

#[test]
fn edits_preserve_secrets_overrides_and_require_restart_for_static_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let text = "[server]\nsession_ttl=600\nlisten='127.0.0.1:8000'\noidc_issuer='https://login.example.test'\noidc_client_id='test'\noidc_client_secret='secret-value'\n[client]\nauthtoken='session-secret'\nurl='https://app.example.test'\n";
    std::fs::write(&path, text).unwrap();
    let overrides = Server {
        listen: Some("127.0.0.1:9000".into()),
        ..Default::default()
    };
    let startup = overrides
        .clone()
        .merge(firemage_config::read(&path, true).unwrap().server);
    let managed = ManagedConfig::new(Some(path.clone()), overrides, startup);
    let before = managed.view().unwrap();
    assert!(!before.toml.contains("secret-value"));
    assert!(!before.toml.contains("session-secret"));
    assert_eq!(before.effective["oidc_client_secret"], "<redacted>");
    let edit = before
        .toml
        .replace("session_ttl = 600", "session_ttl = 900")
        .replace(
            "[server]",
            "[server]\npublic_url = 'https://app.example.test'",
        );
    let preview = managed
        .edit(
            ConfigEdit {
                toml: edit.clone(),
                revision: before.revision.clone(),
            },
            false,
        )
        .unwrap();
    assert_eq!(preview.revision, before.revision);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    let after = managed
        .edit(
            ConfigEdit {
                toml: edit,
                revision: before.revision.clone(),
            },
            true,
        )
        .unwrap();
    assert!(after.restart_required.contains(&"public_url".into()));
    assert!(!after.restart_required.contains(&"session_ttl".into()));
    assert_eq!(managed.snapshot().session_ttl().unwrap(), 900);
    assert_eq!(managed.snapshot().public_url, None);
    assert!(after.effective["public_url"].is_null());
    assert_eq!(
        after.effective_after_restart["public_url"],
        "https://app.example.test"
    );
    assert_eq!(after.effective["session_ttl"], 900);
    assert_eq!(after.effective["listen"], "127.0.0.1:9000");
    let stored = std::fs::read_to_string(&path).unwrap();
    assert!(stored.contains("secret-value") && stored.contains("session-secret"));
    assert!(
        managed
            .edit(
                ConfigEdit {
                    toml: after.toml,
                    revision: before.revision
                },
                true
            )
            .unwrap_err()
            .is::<firemage_config::RevisionConflict>()
    );
}

#[test]
fn invalid_or_external_changes_cannot_overwrite_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[server]\nsession_ttl=600").unwrap();
    let managed = ManagedConfig::new(
        Some(path.clone()),
        Server::default(),
        firemage_config::read(&path, true).unwrap().server,
    );
    let view = managed.view().unwrap();
    assert!(
        managed
            .edit(
                ConfigEdit {
                    toml: "[server]\nsession_ttl=1".into(),
                    revision: view.revision.clone()
                },
                true
            )
            .is_err()
    );
    assert_eq!(managed.snapshot().session_ttl().unwrap(), 600);
    std::fs::write(&path, "[server]\nsession_ttl=1200").unwrap();
    assert!(
        managed
            .edit(
                ConfigEdit {
                    toml: view.toml,
                    revision: view.revision
                },
                true
            )
            .is_err()
    );
    assert!(std::fs::read_to_string(path).unwrap().contains("1200"));
}

#[test]
fn concurrent_edits_have_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[server]\nsession_ttl=600").unwrap();
    let managed = std::sync::Arc::new(ManagedConfig::new(
        Some(path),
        Server::default(),
        Server::default(),
    ));
    let view = managed.view().unwrap();
    let workers: Vec<_> = [900, 1200]
        .into_iter()
        .map(|ttl| {
            let managed = managed.clone();
            let revision = view.revision.clone();
            std::thread::spawn(move || {
                managed
                    .edit(
                        ConfigEdit {
                            toml: format!("[server]\nsession_ttl={ttl}"),
                            revision,
                        },
                        true,
                    )
                    .is_ok()
            })
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .filter(|success| *success)
            .count(),
        1
    );
}

#[test]
fn default_values_do_not_require_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[server]").unwrap();
    let managed = ManagedConfig::new(Some(path), Server::default(), Server::default());
    let view = managed.view().unwrap();
    assert!(view.restart_required.is_empty());
    assert_eq!(view.effective["session_ttl"], 86400);
    assert_eq!(view.effective["listen"], "127.0.0.1:8080");
    let updated = managed
        .edit(
            ConfigEdit {
                toml: "[server]\nlisten='127.0.0.1:8080'\nsession_ttl=86400\ndata_dir='data'"
                    .into(),
                revision: view.revision,
            },
            true,
        )
        .unwrap();
    assert!(updated.restart_required.is_empty());
}

#[test]
fn validation_does_not_create_config_directories_or_lock_files() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("missing");
    let managed = ManagedConfig::new(
        Some(parent.join("config.toml")),
        Server::default(),
        Server::default(),
    );
    let before = managed.view().unwrap();
    managed
        .edit(
            ConfigEdit {
                toml: "[server]\nsession_ttl=600".into(),
                revision: before.revision,
            },
            false,
        )
        .unwrap();
    assert!(!parent.exists());
    assert_eq!(managed.snapshot().session_ttl().unwrap(), 86400);
}

#[test]
fn host_policy_edits_are_rejected_even_when_overridden() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let source = "[server]\nfirecracker='/usr/bin/firecracker'\nallow_trusted_vms=false\n";
    std::fs::write(&path, source).unwrap();
    let overrides = Server {
        firecracker: Some("/opt/firecracker".into()),
        ..Default::default()
    };
    let managed = ManagedConfig::new(
        Some(path.clone()),
        overrides.clone(),
        overrides.merge(firemage_config::read(&path, true).unwrap().server),
    );
    let view = managed.view().unwrap();
    for field in [
        "firecracker",
        "jailer",
        "allow_trusted_vms",
        "oidc_client_secret",
        "unix_socket",
    ] {
        assert!(view.host_only.contains(&field.to_owned()));
    }
    assert!(!view.host_only.contains(&"session_ttl".to_owned()));
    for change in [
        source.replace("/usr/bin/firecracker", "/bin/sh"),
        source.replace("allow_trusted_vms=false", "allow_trusted_vms=true"),
        source.replace("[server]", "[server]\nfirecracker_args=['--no-seccomp']"),
        source.replace("[server]", "[server]\nwebui_dir='/etc'"),
        source.replace("[server]", "[server]\nunix_socket='/run/other.sock'"),
    ] {
        for save in [false, true] {
            let error = managed
                .edit(
                    ConfigEdit {
                        toml: change.clone(),
                        revision: view.revision.clone(),
                    },
                    save,
                )
                .unwrap_err();
            assert!(error.to_string().contains("host-managed"), "{error}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        }
    }
}

#[test]
fn authentication_credentials_and_client_paths_cannot_be_redirected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let source = "[server]\noidc_issuer='https://login.example.test'\noidc_client_id='client'\noidc_client_secret='private-value'\n[client]\nauth_token_path='/var/lib/firemage/token'\n";
    std::fs::write(&path, source).unwrap();
    let managed = ManagedConfig::new(
        Some(path.clone()),
        Server::default(),
        firemage_config::read(&path, true).unwrap().server,
    );
    let view = managed.view().unwrap();
    for change in [
        view.toml
            .replace("login.example.test", "attacker.example.test"),
        view.toml.replace("<redacted>", "replacement"),
        view.toml.replace("/var/lib/firemage/token", "/etc/other"),
    ] {
        let error = managed
            .edit(
                ConfigEdit {
                    toml: change,
                    revision: view.revision.clone(),
                },
                true,
            )
            .unwrap_err();
        assert!(error.to_string().contains("host-managed"), "{error}");
        assert!(!error.to_string().contains("private-value"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }
    managed
        .edit(
            ConfigEdit {
                toml: view.toml,
                revision: view.revision,
            },
            true,
        )
        .unwrap();
}
