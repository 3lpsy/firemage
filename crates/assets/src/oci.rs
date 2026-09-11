use anyhow::Context;
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub(crate) async fn oci(
    image: &str,
    size_mib: u64,
    destination: &Path,
    registry: &crate::RegistryOptions,
    command_override: Option<&[String]>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        (16..=32768).contains(&size_mib),
        "OCI disk size must be 16-32768 MiB"
    );
    let parent = destination
        .parent()
        .context("OCI disk needs a parent directory")?;
    let image = firemage_oci::unpack(
        image,
        parent,
        registry,
        size_mib * 1024 * 1024,
        command_override,
    )
    .await?;
    anyhow::ensure!(
        image.has_file("/bin/sh")?,
        "OCI simple path requires /bin/sh, mount and reboot; supply a bootable rootfs for minimal images"
    );
    let process = image.process();
    anyhow::ensure!(
        process["user"]["uid"].as_u64() == Some(0)
            && process["user"]["gid"].as_u64() == Some(0)
            && process["user"]["username"]
                .as_str()
                .is_none_or(|name| name.is_empty() || name == "root"),
        "OCI non-root USER requires a BYO init"
    );
    let init = crate::oci_init::init(process)?;
    let init_path = image.rootfs().join("firemage-init");
    let mut init_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o755)
        .open(init_path)
        .await
        .context("OCI image reserves /firemage-init")?;
    init_file.write_all(init.as_bytes()).await?;
    init_file.sync_all().await?;
    drop(init_file);
    image.finalize()?;
    crate::ext4(image.rootfs(), destination, size_mib, "rootfs").await
}
