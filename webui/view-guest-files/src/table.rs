use crate::state::{Crumb, Directory, size, visible_name};
use dioxus::prelude::*;
use firemage_webui_component_controls::{CopyButton, CopySource, Icon};
use firemage_webui_provider_api::encode;

#[component]
pub fn Entries(
    id: String,
    directory: Directory,
    path: Vec<Crumb>,
    navigate: EventHandler<Vec<Crumb>>,
) -> Element {
    rsx! {
        p { class: "muted small guest-entry-count", "{directory.entries.len()} entries" }
        div { class: "guest-file-table-wrap",
            table { class: "guest-file-table",
                thead { tr { th { "Name" } th { "Size" } th { "Owner" } th { "Mode" } th { "" } } }
                tbody {
                    for entry in &directory.entries {
                        tr { key: "{entry.inode}-{entry.name}",
                            td {
                                if entry.kind == "directory" {
                                    a { class: "guest-folder-link", href: format!("#vms/{}", encode(&id)),
                                        onclick: { let mut target = path.clone(); target.push(Crumb { inode: entry.inode, name: entry.name.clone() }); move |event| { if event.modifiers().is_empty() { event.prevent_default(); navigate.call(target.clone()); } } },
                                        span { class: "guest-folder-icon", "aria-hidden": "true" }
                                        "{visible_name(&entry.name)}"
                                    }
                                } else {
                                    span { class: "guest-file-name",
                                        Icon { name: "assets", size: 16 }
                                        "{visible_name(&entry.name)}"
                                    }
                                }
                            }
                            td { if let Some(bytes) = entry.size_bytes { "{size(bytes)}" } else { "{entry.kind}" } }
                            td { "{entry.uid}:{entry.gid}" }
                            td { class: "mono", "{entry.mode:04o}" }
                            td {
                                if entry.kind == "file" && entry.size_bytes.is_some_and(|bytes| bytes <= directory.max_file_bytes) {
                                    div { class: "actions",
                                        a { class: "button quiet icon-button", title: "Download file", "aria-label": format!("Download {}", visible_name(&entry.name)), href: format!("/v1/vms/{}/files/download?inode={}&filename={}", encode(&id), entry.inode, encode(&entry.name)), download: visible_name(&entry.name), Icon { name: "download" } }
                                        CopyButton { source: CopySource::File(format!("/v1/vms/{}/files/download?inode={}&filename={}", encode(&id), entry.inode, encode(&entry.name))), label: format!("Copy contents of {}", visible_name(&entry.name)) }
                                    }
                                } else if entry.kind == "file" {
                                    span { class: "muted small", title: format!("Download limit: {}", size(directory.max_file_bytes)), if entry.size_bytes.is_some() { "Exceeds download limit" } else { "Size unavailable" } }
                                } else if entry.kind != "directory" {
                                    span { class: "muted small", "{entry.kind}" }
                                }
                            }
                        }
                    }
                }
            }
        }
        if directory.entries.is_empty() { p { class: "muted", "This directory is empty." } }
    }
}
