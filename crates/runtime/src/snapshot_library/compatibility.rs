use anyhow::Context;
use firemage_firecracker::Firecracker;
use firemage_wire::{IsolationMode, SnapshotManifest, VmSpec};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn ensure_target(manifest: &SnapshotManifest, target: &VmSpec) -> anyhow::Result<()> {
    target.validate()?;
    anyhow::ensure!(
        target.security.mode == IsolationMode::Jailed && target.socket.is_none(),
        "snapshot restore requires a managed jailed VM"
    );
    anyhow::ensure!(
        manifest.architecture == std::env::consts::ARCH,
        "snapshot architecture differs from this host"
    );
    let source = &manifest.spec;
    if manifest.files.contains_key("egress-bootstrap")
        || source.egress_policy.is_some()
        || source
            .egress
            .as_ref()
            .is_some_and(|policy| policy.http.is_some())
    {
        let source_port = source
            .egress
            .as_ref()
            .and_then(|policy| policy.http.as_ref())
            .map(|http| http.port)
            .unwrap_or(source.egress_http_port);
        anyhow::ensure!(
            source_port == target.egress_http_port,
            "snapshot guest HTTP proxy port differs from target VM; use the saved guest-facing port"
        );
    }
    anyhow::ensure!(
        source.vcpus == target.vcpus && source.memory_mib == target.memory_mib,
        "snapshot CPU or memory differs from target VM"
    );
    anyhow::ensure!(
        source.initrd.is_some() == target.initrd.is_some(),
        "snapshot initrd configuration differs from target VM"
    );
    let drives = |spec: &VmSpec| {
        spec.drives
            .iter()
            .map(|drive| (drive.id.clone(), drive.read_only))
            .collect::<BTreeMap<_, _>>()
    };
    anyhow::ensure!(
        drives(source) == drives(target),
        "snapshot drives differ from target VM"
    );
    match (&source.network, &target.network) {
        (None, None) => (),
        (Some(source), Some(target)) => anyhow::ensure!(
            source.address == target.address && source.mac.eq_ignore_ascii_case(&target.mac),
            "snapshot guest IP or MAC differs from target VM"
        ),
        _ => anyhow::bail!("snapshot network interface differs from target VM"),
    }
    Ok(())
}

pub(super) async fn ensure_devices(fc: &Firecracker, spec: &VmSpec) -> anyhow::Result<Value> {
    let config = fc.call("GET", "/vm/config", Value::Null).await?;
    ensure_config(&config, spec)?;
    Ok(config)
}

pub(super) fn ensure_config(config: &Value, spec: &VmSpec) -> anyhow::Result<()> {
    anyhow::ensure!(config.is_object(), "invalid Firecracker configuration");
    for field in ["vsock", "balloon", "cpu-config", "memory-hotplug"] {
        anyhow::ensure!(
            config[field].is_null(),
            "snapshots do not support unmanaged {field} configuration"
        );
    }
    anyhow::ensure!(
        config["pmem"].is_null() || config["pmem"].as_array().is_some_and(Vec::is_empty),
        "snapshots do not support persistent memory devices"
    );
    let machine = &config["machine-config"];
    anyhow::ensure!(
        machine["vcpu_count"] == spec.vcpus && machine["mem_size_mib"] == spec.memory_mib,
        "Firecracker CPU or memory differs from VM definition"
    );
    anyhow::ensure!(
        !machine["smt"].as_bool().unwrap_or(false),
        "snapshots do not support SMT"
    );
    anyhow::ensure!(
        machine["huge_pages"].is_null() || machine["huge_pages"] == "None",
        "snapshots do not support huge-page guests"
    );
    let boot = &config["boot-source"];
    anyhow::ensure!(
        boot["kernel_image_path"] == "/resources/kernel",
        "snapshot kernel is outside managed resources"
    );
    if spec.initrd.is_some() {
        anyhow::ensure!(
            boot["initrd_path"] == "/resources/initrd",
            "snapshot initrd is outside managed resources"
        );
    } else {
        anyhow::ensure!(boot["initrd_path"].is_null(), "unexpected snapshot initrd");
    }
    let disks = config["drives"]
        .as_array()
        .context("missing Firecracker drives")?;
    let mut expected = BTreeMap::from([(
        "rootfs".to_owned(),
        ("/resources/rootfs.ext4".to_owned(), false, true),
    )]);
    for drive in &spec.drives {
        expected.insert(
            drive.id.clone(),
            (
                format!("/resources/drive-{}.img", drive.id),
                drive.read_only,
                false,
            ),
        );
    }
    if disks.iter().any(|disk| disk["drive_id"] == "seed") {
        expected.insert("seed".into(), ("/resources/seed.ext4".into(), true, false));
    }
    anyhow::ensure!(
        disks.len() == expected.len(),
        "snapshot disk count differs from VM definition"
    );
    for disk in disks {
        let id = disk["drive_id"]
            .as_str()
            .context("missing snapshot drive ID")?;
        let (path, read_only, root) = expected.remove(id).context("unexpected snapshot drive")?;
        anyhow::ensure!(
            disk["path_on_host"] == path
                && disk["is_read_only"] == read_only
                && disk["is_root_device"] == root
                && disk["socket"].is_null(),
            "snapshot disk configuration differs from managed resources"
        );
    }
    let nics = config["network-interfaces"]
        .as_array()
        .context("missing Firecracker network interfaces")?;
    match &spec.network {
        None => anyhow::ensure!(nics.is_empty(), "unexpected snapshot network interface"),
        Some(net) => anyhow::ensure!(
            nics.len() == 1
                && nics[0]["iface_id"] == "eth0"
                && nics[0]["guest_mac"]
                    .as_str()
                    .is_some_and(|mac| mac.eq_ignore_ascii_case(&net.mac)),
            "snapshot guest network interface differs from VM definition"
        ),
    }
    Ok(())
}
