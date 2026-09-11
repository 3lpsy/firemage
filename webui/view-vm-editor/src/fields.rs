//! Guided asset, capacity, and network fields.
use dioxus::prelude::*;
#[derive(Clone, Copy, PartialEq)]
pub struct Fields {
    pub attachments: Signal<Vec<firemage_webui_view_asset_attachments::AttachmentForm>>,
    pub name: Signal<String>,
    pub mode: Signal<String>,
    pub isolation: Signal<String>,
    pub socket: Signal<String>,
    pub kernel: Signal<String>,
    pub source: Signal<String>,
    pub rootfs: Signal<String>,
    pub rootfs_sha: Signal<String>,
    pub rootfs_size: Signal<String>,
    pub workload_mode: Signal<String>,
    pub command: Signal<String>,
    pub terminal: Signal<bool>,
    pub web_terminal: Signal<bool>,
    pub shell_command: Signal<String>,
    pub metadata: Signal<String>,
    pub registry: Signal<crate::registry::RegistryForm>,
    pub vcpus: Signal<String>,
    pub memory: Signal<String>,
    pub network: Signal<String>,
    pub address: Signal<String>,
    pub mac: Signal<String>,
    pub userdata: Signal<String>,
    pub boot_args: Signal<String>,
}
