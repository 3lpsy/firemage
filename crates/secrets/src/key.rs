use rand::RngCore;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
use zeroize::Zeroizing;

// Publish a complete key atomically without replacing an existing file.
pub fn load(data_dir: &Path) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join("secrets.key");
    if !path.try_exists()? {
        let mut key = Zeroizing::new([0u8; 32]);
        rand::rngs::OsRng.fill_bytes(key.as_mut());
        let mut temporary = tempfile::NamedTempFile::new_in(data_dir)?;
        temporary
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        temporary.write_all(key.as_ref())?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => anyhow::bail!("cannot initialize secret encryption key"),
        }
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)?;
    let meta = file.metadata()?;
    anyhow::ensure!(
        meta.is_file() && meta.len() == 32 && meta.permissions().mode() & 0o077 == 0,
        "secret encryption key must be a private 32-byte regular file"
    );
    let mut key = Zeroizing::new([0u8; 32]);
    file.read_exact(key.as_mut())?;
    Ok(key)
}
