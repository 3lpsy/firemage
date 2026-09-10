use anyhow::Context;
use std::path::Path;

pub(crate) async fn oci(image: &str, size_mib: u64, destination: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        !image.starts_with('-')
            && image
                .split_once("@sha256:")
                .is_some_and(|(name, digest)| !name.is_empty()
                    && digest.len() == 64
                    && digest.bytes().all(|b| b.is_ascii_hexdigit())),
        "OCI image must be pinned with @sha256:<digest>"
    );
    let layout = destination.with_extension("oci");
    let bundle = destination.with_extension("bundle");
    if layout.exists() {
        tokio::fs::remove_dir_all(&layout).await?;
    }
    if bundle.exists() {
        tokio::fs::remove_dir_all(&bundle).await?;
    }
    let layout_s = layout.to_str().context("invalid OCI path")?;
    let bundle_s = bundle.to_str().context("invalid bundle path")?;
    crate::command(
        "skopeo",
        &[
            "copy",
            &format!("docker://{image}"),
            &format!("oci:{layout_s}:rootfs"),
        ],
    )
    .await?;
    crate::command(
        "umoci",
        &[
            "unpack",
            "--rootless",
            "--image",
            &format!("{layout_s}:rootfs"),
            bundle_s,
        ],
    )
    .await?;
    let config: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(bundle.join("config.json")).await?)?;
    let process = &config["process"];
    let root = bundle.join("rootfs");
    anyhow::ensure!(
        root.join("bin/sh").exists(),
        "OCI simple path requires /bin/sh, mount and reboot; supply a bootable rootfs for minimal images"
    );
    anyhow::ensure!(
        process["user"]["uid"].as_u64().unwrap_or(0) == 0
            && process["user"]["gid"].as_u64().unwrap_or(0) == 0,
        "OCI non-root USER requires a BYO init"
    );
    let init = crate::oci_init::init(process)?;
    let init_path = root.join("firemage-init");
    anyhow::ensure!(
        !init_path.symlink_metadata().is_ok(),
        "OCI image reserves /firemage-init"
    );
    tokio::fs::write(&init_path, init).await?;
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(init_path, std::fs::Permissions::from_mode(0o755)).await?;
    let result = crate::ext4(&root, destination, size_mib, "rootfs").await;
    let _ = tokio::fs::remove_dir_all(&layout).await;
    let _ = tokio::fs::remove_dir_all(&bundle).await;
    result
}
