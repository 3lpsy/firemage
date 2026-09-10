use clap::Parser;
use firemage_config::{Client, Config, Server};
use std::os::unix::fs::{PermissionsExt, symlink};

#[derive(Parser)]
struct Options {
    #[command(flatten)]
    server: Server,
    #[command(flatten)]
    client: Client,
}

#[test]
fn cli_overrides_environment_then_toml() {
    // Use a subprocess so environment changes cannot race other Rust tests.
    if std::env::var_os("FIREMAGE_PRECEDENCE_TEST").is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("FIREMAGE_") {
                child.env_remove(name);
            }
        }
        let result = child
            .args([
                "--exact",
                "cli_overrides_environment_then_toml",
                "--nocapture",
            ])
            .env("FIREMAGE_PRECEDENCE_TEST", "1")
            .env("FIREMAGE_LISTEN", "127.0.0.1:9001")
            .env("FIREMAGE_URL", "https://env.example.test")
            .env("FIREMAGE_AUTHTOKEN", "session-env-test")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let file: Config = toml::from_str(
        r#"
        [server]
        listen = "127.0.0.1:9000"
        session_ttl = 600
        [client]
        url = "https://file.example.test"
        auth_token_path = "/tmp/test-token"
    "#,
    )
    .unwrap();
    let env = Options::try_parse_from(["firemage"]).unwrap();
    let server = env.server.merge(file.server.clone());
    let client = env.client.merge(file.client.clone());
    assert_eq!(server.listen.as_deref(), Some("127.0.0.1:9001"));
    assert_eq!(server.session_ttl().unwrap(), 600);
    assert_eq!(client.url.as_deref(), Some("https://env.example.test"));
    assert_eq!(
        client.auth_token_path.unwrap(),
        std::path::Path::new("/tmp/test-token")
    );
    assert_eq!(client.authtoken.as_deref(), Some("session-env-test"));
    let cli = Options::try_parse_from([
        "firemage",
        "--listen",
        "127.0.0.1:9002",
        "--url",
        "https://cli.example.test",
        "--authtoken",
        "session-cli-test",
    ])
    .unwrap();
    assert_eq!(
        cli.server.merge(file.server).listen.as_deref(),
        Some("127.0.0.1:9002")
    );
    let client = cli.client.merge(file.client);
    assert_eq!(client.url.as_deref(), Some("https://cli.example.test"));
    assert_eq!(client.authtoken.as_deref(), Some("session-cli-test"));
    let serialized = toml::to_string(&client).unwrap();
    assert!(!serialized.contains("session-cli-test"));
    assert!(!serialized.contains("authtoken"));
}

#[test]
fn credential_replacement_is_private_and_does_not_follow_symlinks() {
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let token = directory.path().join("session.toml");
    let other = directory.path().join("unrelated.txt");
    std::fs::write(&other, "keep").unwrap();
    symlink(&other, &token).unwrap();
    firemage_config::write_private(&token, b"first-secret").unwrap();
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "keep");
    assert!(
        !std::fs::symlink_metadata(&token)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        std::fs::metadata(&token).unwrap().permissions().mode() & 0o777,
        0o600
    );
    firemage_config::write_private(&token, b"second-secret").unwrap();
    assert_eq!(std::fs::read(&token).unwrap(), b"second-secret");
    symlink(&other, token.with_extension("tmp")).unwrap();
    assert!(firemage_config::write_private(&token, b"third-secret").is_err());
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "keep");
    assert_eq!(std::fs::read(&token).unwrap(), b"second-secret");
}

#[test]
fn configuration_rejects_unknown_fields_and_invalid_session_lifetimes() {
    assert!(toml::from_str::<Config>("[server]\nsession_ttl_typo = 600").is_err());
    for ttl in [0, 59, 2_592_001, u64::MAX] {
        assert!(
            Server {
                session_ttl: Some(ttl),
                ..Default::default()
            }
            .session_ttl()
            .is_err()
        );
    }
    let directory = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
    let missing = directory.path().join("missing.toml");
    assert!(firemage_config::read(&missing, false).is_ok());
    assert!(firemage_config::read(&missing, true).is_err());
}

#[test]
fn custom_oidc_ca_is_configurable_and_requires_an_issuer() {
    let file: Config = toml::from_str("[server]\noidc_issuer='https://identity.example.test'\noidc_client_id='firemage'\noidc_ca_cert='/etc/identity-ca.pem'").unwrap();
    let cli =
        Options::try_parse_from(["firemage", "--oidc-ca-cert", "/etc/override-ca.pem"]).unwrap();
    let resolved = cli.server.merge(file.server);
    resolved.validate().unwrap();
    assert_eq!(resolved.oidc_ca_cert, Some("/etc/override-ca.pem".into()));
    assert!(
        Server {
            oidc_ca_cert: Some("/etc/identity-ca.pem".into()),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
