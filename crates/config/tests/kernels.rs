use clap::Parser;
use firemage_config::{Config, Server};

#[derive(Parser)]
struct Options {
    #[command(flatten)]
    server: Server,
}

#[test]
fn kernel_directory_precedence_and_host_confinement() {
    if std::env::var_os("FIREMAGE_KERNEL_PRECEDENCE_TEST").is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("FIREMAGE_") {
                child.env_remove(name);
            }
        }
        let output = child
            .args([
                "--exact",
                "kernel_directory_precedence_and_host_confinement",
            ])
            .env("FIREMAGE_KERNEL_PRECEDENCE_TEST", "1")
            .env("FIREMAGE_KERNEL_DIR", "/srv/environment/kernels")
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
    assert_eq!(
        Server::default().kernel_dir(),
        std::path::Path::new("/var/lib/firemage/kernels")
    );
    let file: Config = toml::from_str("[server]\nkernel_dir='/srv/file/kernels'").unwrap();
    assert_eq!(
        Options::try_parse_from(["firemage"])
            .unwrap()
            .server
            .merge(file.server.clone())
            .kernel_dir(),
        std::path::Path::new("/srv/environment/kernels")
    );
    assert_eq!(
        Options::try_parse_from(["firemage", "--kernel-dir", "/srv/cli/kernels"])
            .unwrap()
            .server
            .merge(file.server)
            .kernel_dir(),
        std::path::Path::new("/srv/cli/kernels")
    );
    for path in ["", "relative", "/srv/../kernels"] {
        assert!(
            Server {
                kernel_dir: Some(path.into()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    std::fs::write(&path, "[server]\nkernel_dir='/srv/kernels'").unwrap();
    let startup = firemage_config::read(&path, true).unwrap().server;
    let managed = firemage_config::ManagedConfig::new(Some(path), Server::default(), startup);
    let view = managed.view().unwrap();
    assert!(view.host_only.contains(&"kernel_dir".into()));
    let error = managed
        .edit(
            firemage_config::ConfigEdit {
                toml: view.toml.replace("/srv/kernels", "/etc"),
                revision: view.revision,
            },
            true,
        )
        .unwrap_err();
    assert!(error.to_string().contains("host-managed"));
}
