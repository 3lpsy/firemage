use anyhow::ensure;
use firemage_wire::{IsolationMode, SnapshotManifest};
use std::collections::BTreeSet;

pub fn validate_manifest(manifest: &SnapshotManifest, limit: u64) -> anyhow::Result<()> {
    ensure!(manifest.version == 1, "unsupported snapshot bundle version");
    firemage_wire::ensure_name(&manifest.source_vm_name)?;
    ensure!(
        matches!(manifest.architecture.as_str(), "x86_64" | "aarch64"),
        "unsupported snapshot architecture"
    );
    ensure!(
        !manifest.firecracker_version.is_empty()
            && manifest.firecracker_version.len() <= 64
            && manifest
                .firecracker_version
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".+-".contains(&c)),
        "invalid Firecracker version"
    );
    manifest.spec.validate()?;
    ensure!(
        manifest.spec.security.mode == IsolationMode::Jailed && manifest.spec.socket.is_none(),
        "portable snapshots require a managed jailed VM"
    );
    let mut required: BTreeSet<String> = ["state.bin", "memory.bin", "kernel", "rootfs.ext4"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    if manifest.spec.initrd.is_some() {
        required.insert("initrd".into());
    }
    required.extend(
        manifest
            .spec
            .drives
            .iter()
            .map(|d| format!("drive-{}.img", d.id)),
    );
    ensure!(
        required
            .iter()
            .all(|name| manifest.files.contains_key(name)),
        "snapshot bundle is missing required VM files"
    );
    required.extend(["seed.ext4".into(), "oci-init-version".into()]);
    ensure!(
        manifest.files.keys().all(|name| required.contains(name)),
        "snapshot bundle contains unexpected files"
    );
    ensure!(
        manifest.files["state.bin"].size_bytes <= 64 * 1024 * 1024,
        "snapshot state exceeds 64 MiB"
    );
    ensure!(
        manifest.files["kernel"].size_bytes <= 128 * 1024 * 1024,
        "snapshot kernel exceeds 128 MiB"
    );
    ensure!(
        manifest.files["memory.bin"].size_bytes
            == u64::from(manifest.spec.memory_mib) * 1024 * 1024,
        "snapshot memory size does not match VM memory"
    );
    let mut total = 0u64;
    for file in manifest.files.values() {
        ensure!(
            file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid snapshot file hash"
        );
        total = total
            .checked_add(file.size_bytes)
            .ok_or_else(|| anyhow::anyhow!("snapshot size overflow"))?;
        ensure!(
            total <= limit,
            "expanded snapshot exceeds configured size limit"
        );
    }
    match (
        &manifest.spec.network,
        &manifest.network,
        &manifest.gateway_mac,
    ) {
        (None, None, None) => (),
        (Some(attachment), Some(network), Some(mac)) => {
            network.validate()?;
            ensure!(
                attachment.network == network.name
                    && network.subnet.contains(&attachment.address)
                    && attachment.address != network.gateway,
                "snapshot network does not match guest attachment"
            );
            let mut spec = manifest.spec.clone();
            spec.network.as_mut().unwrap().mac = mac.clone();
            spec.validate()?;
        }
        _ => anyhow::bail!("snapshot network metadata is incomplete"),
    }
    Ok(())
}
