use std::{
    fs::File,
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::Path,
};

// Called under the VM lock after stop; the next jailed start reassigns the disk UID.
pub(crate) fn claim_stopped_disk(path: &Path) -> anyhow::Result<File> {
    anyhow::ensure!(
        unsafe { libc::geteuid() } == 0,
        "jailed output extraction requires the root host service"
    );
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "guest output disk must be a regular file"
    );
    anyhow::ensure!(
        unsafe { libc::fchown(file.as_raw_fd(), 0, 0) } == 0,
        "cannot reclaim stopped guest disk: {}",
        std::io::Error::last_os_error()
    );
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    Ok(file)
}
