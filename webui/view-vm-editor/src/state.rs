use crate::fields::Fields;
use crate::registry;
use dioxus::prelude::*;
use firemage_webui_provider_api::text;
use serde_json::Value;

pub fn use_fields(initial: &Value) -> Fields {
    let name = use_signal(|| text(initial, "name"));
    let mode = use_signal(|| {
        if initial["socket"].is_string() {
            "socket".into()
        } else {
            "managed".into()
        }
    });
    let isolation = use_signal(|| {
        initial["security"]["mode"]
            .as_str()
            .unwrap_or("jailed")
            .to_owned()
    });
    let socket = use_signal(|| text(initial, "socket"));
    let kernel = use_signal(|| {
        initial["kernel"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    });
    let source = use_signal(|| {
        initial["rootfs"]["kind"]
            .as_str()
            .unwrap_or("local")
            .to_owned()
    });
    let rootfs = use_signal(|| {
        initial["rootfs"]["path"]
            .as_str()
            .or(initial["rootfs"]["url"].as_str())
            .or(initial["rootfs"]["image"].as_str())
            .unwrap_or_default()
            .to_owned()
    });
    let registry =
        use_signal(|| registry::RegistryForm::from_value(&initial["rootfs"]["registry"]));
    let rootfs_sha = use_signal(|| text(&initial["rootfs"], "sha256"));
    let rootfs_size = use_signal(|| {
        initial["rootfs"]["size_mib"]
            .as_u64()
            .unwrap_or(2048)
            .to_string()
    });
    let workload_mode = use_signal(|| {
        initial["workload"]["mode"]
            .as_str()
            .unwrap_or("one-shot")
            .to_owned()
    });
    let command = use_signal(|| optional_json(&initial["workload"]["command"]));
    let terminal = use_signal(|| initial["terminal"].as_bool().unwrap_or(false));
    let metadata = use_signal(|| optional_json(&initial["metadata"]));
    let vcpus = use_signal(|| initial["vcpus"].as_u64().unwrap_or(1).to_string());
    let memory = use_signal(|| initial["memory_mib"].as_u64().unwrap_or(256).to_string());
    let network = use_signal(|| text(&initial["network"], "network"));
    let address = use_signal(|| text(&initial["network"], "address"));
    let mac = use_signal(|| {
        initial["network"]["mac"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    });
    let userdata = use_signal(|| text(initial, "userdata"));
    let boot_args = use_signal(|| {
        initial["boot_args"]
            .as_str()
            .unwrap_or("console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw")
            .to_owned()
    });
    let attachments = use_signal(|| firemage_webui_view_asset_attachments::from_spec(initial));
    Fields {
        name,
        mode,
        isolation,
        socket,
        kernel,
        source,
        rootfs,
        registry,
        rootfs_sha,
        rootfs_size,
        workload_mode,
        command,
        terminal,
        metadata,
        vcpus,
        memory,
        network,
        address,
        mac,
        userdata,
        boot_args,
        attachments,
    }
}
impl Fields {
    pub fn form(self) -> crate::spec::Form {
        crate::spec::Form {
            name: (self.name)(),
            mode: (self.mode)(),
            isolation: (self.isolation)(),
            socket: (self.socket)(),
            kernel: (self.kernel)(),
            source: (self.source)(),
            rootfs: (self.rootfs)(),
            registry: (self.registry)(),
            rootfs_sha: (self.rootfs_sha)(),
            rootfs_size: (self.rootfs_size)(),
            workload_mode: (self.workload_mode)(),
            command: (self.command)(),
            terminal: (self.terminal)(),
            metadata: (self.metadata)(),
            vcpus: (self.vcpus)(),
            memory: (self.memory)(),
            network: (self.network)(),
            address: (self.address)(),
            mac: (self.mac)(),
            userdata: (self.userdata)(),
            boot_args: (self.boot_args)(),
        }
    }
    pub fn load(mut self, value: &Value) {
        self.name.set(text(value, "name"));
        self.userdata.set(text(value, "userdata"));
        self.boot_args.set(text(value, "boot_args"));
        self.mode.set(
            if value["socket"].is_string() {
                "socket"
            } else {
                "managed"
            }
            .into(),
        );
        self.isolation.set(
            value["security"]["mode"]
                .as_str()
                .unwrap_or("jailed")
                .into(),
        );
        self.socket.set(text(value, "socket"));
        self.kernel.set(text(&value["kernel"], "name"));
        self.source
            .set(value["rootfs"]["kind"].as_str().unwrap_or("local").into());
        self.rootfs.set(
            value["rootfs"]["path"]
                .as_str()
                .or(value["rootfs"]["url"].as_str())
                .or(value["rootfs"]["image"].as_str())
                .unwrap_or_default()
                .into(),
        );
        self.rootfs_sha.set(text(&value["rootfs"], "sha256"));
        self.rootfs_size.set(
            value["rootfs"]["size_mib"]
                .as_u64()
                .unwrap_or(2048)
                .to_string(),
        );
        self.workload_mode.set(
            value["workload"]["mode"]
                .as_str()
                .unwrap_or("one-shot")
                .into(),
        );
        self.command
            .set(optional_json(&value["workload"]["command"]));
        self.terminal
            .set(value["terminal"].as_bool().unwrap_or(false));
        self.metadata.set(optional_json(&value["metadata"]));
        self.registry.set(registry::RegistryForm::from_value(
            &value["rootfs"]["registry"],
        ));
        self.vcpus
            .set(value["vcpus"].as_u64().unwrap_or(1).to_string());
        self.memory
            .set(value["memory_mib"].as_u64().unwrap_or(256).to_string());
        self.network.set(text(&value["network"], "network"));
        self.address.set(text(&value["network"], "address"));
        self.mac
            .set(value["network"]["mac"].as_str().unwrap_or_default().into());
        self.attachments
            .set(firemage_webui_view_asset_attachments::from_spec(value));
    }
}

fn optional_json(value: &Value) -> String {
    if value.is_null() {
        String::new()
    } else {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
}
