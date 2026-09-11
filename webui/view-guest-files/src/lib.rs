mod state;
mod table;
mod tree;

use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use serde_json::Value;
use state::{BrowserState, Crumb, load, visible_name};

#[component]
pub fn GuestFiles(vm: Value) -> Element {
    let id = vm["id"].as_str().unwrap_or_default().to_owned();
    if vm["state"].as_str() != Some("stopped") {
        return rsx! { p { class: "muted", "Stop the VM to browse and download files from its root disk." } };
    }
    rsx! { Browser { key: "files-{id}", id } }
}

#[component]
fn Browser(id: String) -> Element {
    let mut state = use_signal(BrowserState::default);
    let mut tree_open = use_signal(|| false);
    let initial_id = id.clone();
    use_future(move || load(initial_id.clone(), 2, state));
    let navigate_id = id.clone();
    let navigate = EventHandler::new(move |path: Vec<Crumb>| {
        let inode = path.last().map_or(2, |crumb| crumb.inode);
        {
            let mut current = state.write();
            current
                .expanded
                .extend(path.iter().map(|crumb| crumb.inode));
            current.path = path;
        }
        spawn(load(navigate_id.clone(), inode, state));
    });
    let (path, inode, directory, error, loading) = {
        let current = state.read();
        let inode = current.path.last().map_or(2, |crumb| crumb.inode);
        (
            current.path.clone(),
            inode,
            current.directories.get(&inode).cloned(),
            current.errors.get(&inode).cloned(),
            current.loading.contains(&inode),
        )
    };
    let refresh_id = id.clone();
    rsx! {
        style { {include_str!("style.css")} }
        div { class: "guest-files",
            div { class: "heading compact",
                h3 { "Files" }
                div { class: "actions",
                    button { class: "guest-tree-toggle", onclick: move |_| tree_open.toggle(), "Directories" }
                    button {
                        disabled: loading,
                        onclick: move |_| {
                            state.write().directories.remove(&inode);
                            state.write().errors.remove(&inode);
                            spawn(load(refresh_id.clone(), inode, state));
                        }, "Refresh"
                    }
                }
            }
            div { class: if tree_open() { "guest-files-layout tree-open" } else { "guest-files-layout" },
                nav { class: "guest-directory-tree", "aria-label": "Guest directories",
                    h4 { "Root disk" }
                    tree::Folder { node: Crumb { inode: 2, name: "/".into() }, parents: vec![], state, navigate }
                }
                div { class: "guest-files-main",
                    nav { class: "guest-file-breadcrumbs", "aria-label": "Guest file path",
                        for (index, crumb) in path.iter().enumerate() {
                            if index > 1 { span { " / " } }
                            button {
                                class: "quiet",
                                disabled: index + 1 == path.len(),
                                onclick: { let path = path[..=index].to_vec(); move |_| navigate.call(path.clone()) },
                                "{visible_name(&crumb.name)}"
                            }
                        }
                    }
                    if let Some(error) = error { Notice { message: error } }
                    if let Some(directory) = directory {
                        table::Entries { id, directory, path: path, navigate }
                    } else if loading {
                        p { class: "muted", role: "status", "Loading directory…" }
                    }
                    p { class: "muted small guest-files-note", "Browse and download files while the VM is stopped." }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
