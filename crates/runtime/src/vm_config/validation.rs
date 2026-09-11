use anyhow::ensure;
use firemage_wire::{Asset, VmConfigDocument};

pub(super) fn ensure_portable(document: &VmConfigDocument) -> anyhow::Result<()> {
    ensure!(document.version == 1, "unsupported VM config version");
    ensure!(
        document.vm.kernel.is_none()
            && document.vm.attachments.is_empty()
            && document.vm.egress_policy.is_none(),
        "use kernel, asset and egress policy aliases, not catalog filenames or IDs"
    );
    ensure!(
        document.vm.socket.is_none(),
        "external socket configurations cannot be exported or imported"
    );
    for asset in document
        .vm
        .rootfs
        .iter()
        .chain(document.vm.initrd.iter())
        .chain(document.vm.drives.iter().map(|drive| &drive.asset))
    {
        ensure!(
            !matches!(asset, Asset::Local { .. } | Asset::Kernel { .. }),
            "local disk paths cannot be exported or imported; use an OCI image or verified remote disk"
        );
    }
    if let Some(alias) = &document.kernel_alias {
        firemage_wire::ensure_asset_alias(alias)?;
    }
    if let Some(alias) = &document.egress_policy_alias {
        firemage_wire::ensure_asset_alias(alias)?;
        ensure!(
            document.vm.egress.is_none(),
            "choose an egress policy alias or legacy inline policy, not both"
        );
    }
    for attachment in &document.attachments {
        firemage_wire::ensure_asset_alias(&attachment.alias)?;
    }
    Ok(())
}
