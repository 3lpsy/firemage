use anyhow::Context;
use std::{
    collections::BTreeSet,
    fs::File,
    os::fd::AsRawFd,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tokio::process::Command;

pub(crate) struct Sandbox {
    pub command: Command,
    _filter: File,
}

pub(crate) async fn command(disk: &Path, guest_path: &str) -> anyhow::Result<Sandbox> {
    firemage_wire::ensure_guest_path(guest_path)?;
    command_request(disk, &format!("cat /{guest_path}")).await
}

pub(crate) async fn command_request(disk: &Path, request: &str) -> anyhow::Result<Sandbox> {
    let executable = host_executable(&["/usr/sbin/debugfs", "/sbin/debugfs", "/usr/bin/debugfs"])?;
    let bwrap = host_executable(&["/usr/bin/bwrap", "/bin/bwrap"])?;
    let dependencies = dependencies(&executable).await?;
    let disk = std::fs::canonicalize(disk).context("opening guest disk for isolated extraction")?;
    anyhow::ensure!(disk.is_file(), "guest disk must be a regular file");
    let filter = super::security::filter()?;
    let mut command = build(
        &bwrap,
        &executable,
        &dependencies,
        &disk,
        filter.as_raw_fd(),
    );
    command.args(["-R", request, "/input/rootfs.ext4"]);
    Ok(Sandbox {
        command,
        _filter: filter,
    })
}

pub(super) fn build(
    bwrap: &Path,
    executable: &Path,
    dependencies: &[PathBuf],
    disk: &Path,
    filter: i32,
) -> Command {
    let mut command = Command::new(bwrap);
    command.env_clear().args([
        "--unshare-user",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup",
        "--uid",
        "65534",
        "--gid",
        "65534",
        "--cap-drop",
        "ALL",
        "--disable-userns",
        "--new-session",
        "--die-with-parent",
        "--clearenv",
        "--setenv",
        "LC_ALL",
        "C",
        "--chdir",
        "/",
    ]);
    command.arg("--ro-bind").arg(executable).arg("/debugfs");
    for dependency in dependencies {
        command.arg("--ro-bind").arg(dependency).arg(dependency);
    }
    command.arg("--ro-bind").arg(disk).arg("/input/rootfs.ext4");
    command.args([
        "--remount-ro",
        "/",
        "--seccomp",
        &filter.to_string(),
        "--",
        "/debugfs",
    ]);
    unsafe {
        command.pre_exec(move || super::security::limits(filter));
    }
    command
}

fn host_executable(candidates: &[&str]) -> anyhow::Result<PathBuf> {
    let path = candidates
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .context("jailed output extraction requires host-installed bubblewrap, debugfs and ldd")?;
    trusted_path(path)
}

fn trusted_path(path: &Path) -> anyhow::Result<PathBuf> {
    let resolved = std::fs::canonicalize(path)?;
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        anyhow::ensure!(
            metadata.uid() == 0 && (metadata.is_symlink() || metadata.mode() & 0o022 == 0),
            "extraction dependency paths must not be replaceable by unprivileged users"
        );
    }
    for ancestor in resolved.ancestors() {
        let metadata = std::fs::metadata(ancestor)?;
        anyhow::ensure!(
            metadata.uid() == 0 && metadata.mode() & 0o022 == 0,
            "extraction executable and library paths must be root-owned and not writable by other users"
        );
    }
    anyhow::ensure!(
        resolved.is_file(),
        "extraction dependency must be a regular file"
    );
    Ok(resolved)
}

async fn dependencies(binary: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let ldd = host_executable(&["/usr/bin/ldd", "/bin/ldd"])?;
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Command::new(ldd)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .arg(binary)
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    anyhow::ensure!(
        output.status.success(),
        "cannot resolve debugfs shared libraries for isolated extraction"
    );
    let text = std::str::from_utf8(&output.stdout)?;
    anyhow::ensure!(
        !text.contains("not found"),
        "debugfs shared library is unavailable"
    );
    let mut paths = BTreeSet::new();
    for word in text.split_whitespace().filter(|word| word.starts_with('/')) {
        let path = PathBuf::from(word);
        anyhow::ensure!(
            ["/lib", "/lib64", "/usr/lib", "/usr/lib64"]
                .iter()
                .any(|root| path.starts_with(root)),
            "debugfs shared libraries must be installed in system library directories"
        );
        trusted_path(&path)?;
        paths.insert(path);
    }
    anyhow::ensure!(
        !paths.is_empty(),
        "debugfs requires a dynamically linked system installation"
    );
    Ok(paths.into_iter().collect())
}
