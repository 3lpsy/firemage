use clap::Parser;
use firemage_config::{Config, Server};

#[derive(Parser)]
struct Options {
    #[command(flatten)]
    server: Server,
}

#[test]
fn socket_permissions_require_explicit_group_and_safe_modes() {
    let default = Server::default();
    assert_eq!(default.unix_socket_mode().unwrap(), 0o600);
    for config in [
        "unix_socket_mode='0666'",
        "unix_socket_mode='0770'",
        "unix_socket_mode='0660'",
        "unix_socket_gid=4294967295",
    ] {
        let parsed: Config = toml::from_str(&format!(
            "[server]\nunix_socket='/run/firemage/api.sock'\n{config}"
        ))
        .unwrap();
        assert!(parsed.server.validate().is_err());
    }
    let orphaned: Config = toml::from_str("[server]\nunix_socket_gid=2000").unwrap();
    assert!(orphaned.server.validate().is_err());
    let valid: Config = toml::from_str("[server]\nunix_socket='/run/firemage/api.sock'\nunix_socket_mode='0660'\nunix_socket_gid=2000").unwrap();
    valid.server.validate().unwrap();
}

#[test]
fn socket_options_follow_cli_environment_toml_precedence() {
    if std::env::var_os("FIREMAGE_SOCKET_PRECEDENCE_TEST").is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("FIREMAGE_") {
                child.env_remove(name);
            }
        }
        let output = child
            .args([
                "--exact",
                "socket_options_follow_cli_environment_toml_precedence",
            ])
            .env("FIREMAGE_SOCKET_PRECEDENCE_TEST", "1")
            .env("FIREMAGE_UNIX_SOCKET", "/run/firemage/env.sock")
            .env("FIREMAGE_UNIX_SOCKET_MODE", "0660")
            .env("FIREMAGE_UNIX_SOCKET_GID", "2001")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let file: Config = toml::from_str("[server]\nunix_socket='/run/firemage/file.sock'\nunix_socket_mode='0600'\nunix_socket_gid=2000").unwrap();
    let env = Options::try_parse_from(["firemage"])
        .unwrap()
        .server
        .merge(file.server.clone());
    env.validate().unwrap();
    assert_eq!(
        env.unix_socket.unwrap().to_str(),
        Some("/run/firemage/env.sock")
    );
    assert_eq!(env.unix_socket_mode.as_deref(), Some("0660"));
    assert_eq!(env.unix_socket_gid, Some(2001));
    let cli = Options::try_parse_from([
        "firemage",
        "--unix-socket",
        "/run/firemage/cli.sock",
        "--unix-socket-mode",
        "0600",
        "--unix-socket-gid",
        "2002",
    ])
    .unwrap()
    .server
    .merge(file.server);
    cli.validate().unwrap();
    assert_eq!(cli.unix_socket_mode().unwrap(), 0o600);
    assert_eq!(cli.unix_socket_gid, Some(2002));
    assert_eq!(
        cli.unix_socket.unwrap().to_str(),
        Some("/run/firemage/cli.sock")
    );
}

#[test]
fn socket_permissions_are_host_only_in_managed_configuration() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::write(
        &path,
        "[server]\nunix_socket='/run/firemage/api.sock'\nunix_socket_mode='0600'",
    )
    .unwrap();
    let startup = firemage_config::read(&path, true).unwrap().server;
    let managed = firemage_config::ManagedConfig::new(Some(path), Server::default(), startup);
    let view = managed.view().unwrap();
    assert!(view.host_only.contains(&"unix_socket_mode".into()));
    assert!(view.host_only.contains(&"unix_socket_gid".into()));
    let edit = firemage_config::ConfigEdit {
        toml: view.toml.replace("0600", "0660") + "unix_socket_gid = 2000\n",
        revision: view.revision,
    };
    assert!(
        managed
            .edit(edit, true)
            .unwrap_err()
            .to_string()
            .contains("host-managed")
    );
}
