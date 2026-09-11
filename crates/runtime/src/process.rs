use anyhow::Context;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

pub(crate) fn identity(pid: i32) -> anyhow::Result<String> {
    Ok(process_identity(pid)?.0)
}
fn process_identity(pid: i32) -> anyhow::Result<(String, bool)> {
    anyhow::ensure!(pid > 1, "invalid Firecracker process id");
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let fields = stat.rsplit_once(") ").context("invalid process stat")?.1;
    let is_running = !matches!(fields.split_whitespace().next(), Some("Z" | "X" | "x"));
    let started = fields
        .split_whitespace()
        .nth(19)
        .context("missing process start time")?;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    Ok((format!("{}:{started}", boot.trim()), is_running))
}
pub(crate) fn is_alive(pid: i32, expected: &str) -> bool {
    process_identity(pid).is_ok_and(|(actual, is_running)| is_running && actual == expected)
}

// A pidfd pins process identity, so a recycled PID cannot receive the stop signal.
pub(crate) async fn stop(pid: i32, expected: String) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        anyhow::ensure!(pid > 1, "invalid process id");
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
        anyhow::ensure!(
            fd >= 0,
            "cannot open Firecracker pidfd: {}",
            std::io::Error::last_os_error()
        );
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        anyhow::ensure!(
            identity(pid)? == expected,
            "Firecracker process identity changed"
        );
        let status = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        anyhow::ensure!(
            status == 0,
            "cannot stop Firecracker: {}",
            std::io::Error::last_os_error()
        );
        let mut poll = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, 5000) };
        anyhow::ensure!(result > 0, "timed out waiting for Firecracker exit");
        Ok(())
    })
    .await?
}

pub(crate) async fn socket_identity(socket: &str) -> anyhow::Result<(i32, String)> {
    let stream = tokio::net::UnixStream::connect(socket).await?;
    let pid = stream
        .peer_cred()?
        .pid()
        .context("socket has no peer process id")?;
    Ok((pid, identity(pid)?))
}
pub(crate) async fn from_socket(socket: &str) -> anyhow::Result<(i32, String)> {
    let (pid, start) = socket_identity(socket).await?;
    let command = std::fs::read(format!("/proc/{pid}/cmdline"))?;
    let arguments: Vec<_> = command.split(|b| *b == 0).collect();
    anyhow::ensure!(
        arguments.windows(2).any(|pair| {
            if pair[0] != b"--api-sock" {
                return false;
            }
            let Ok(argument) = std::str::from_utf8(pair[1]) else {
                return false;
            };
            let directory = if std::path::Path::new(argument).is_absolute() {
                "root"
            } else {
                "cwd"
            };
            let path = std::path::Path::new(&format!("/proc/{pid}/{directory}"))
                .join(argument.trim_start_matches('/'));
            is_same_file(&path, std::path::Path::new(socket)).unwrap_or(false)
        }),
        "socket peer is not the assigned Firecracker process"
    );
    Ok((pid, start))
}

// Procfs magic links can display namespace-relative names; compare their actual filesystem objects.
pub(crate) fn is_same_file(
    left: &std::path::Path,
    right: &std::path::Path,
) -> anyhow::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let left = std::fs::metadata(left)?;
    let right = std::fs::metadata(right)?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}
