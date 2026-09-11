use crate::state::{BrowserState, Crumb, visible_name};
use dioxus::prelude::*;

#[component]
pub fn Folder(
    node: Crumb,
    parents: Vec<Crumb>,
    mut state: Signal<BrowserState>,
    navigate: EventHandler<Vec<Crumb>>,
) -> Element {
    if parents.len() >= 64 || parents.iter().any(|parent| parent.inode == node.inode) {
        return rsx! {};
    }
    let (expanded, selected, children) = {
        let current = state.read();
        let expanded = current.expanded.contains(&node.inode);
        let selected = current
            .path
            .last()
            .is_some_and(|crumb| crumb.inode == node.inode);
        let children = current
            .directories
            .get(&node.inode)
            .filter(|_| expanded)
            .map(|directory| {
                directory
                    .entries
                    .iter()
                    .filter(|entry| entry.kind == "directory")
                    .map(|entry| Crumb {
                        inode: entry.inode,
                        name: entry.name.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        (expanded, selected, children)
    };
    let mut path = parents.clone();
    path.push(node.clone());
    let inode = node.inode;
    let open_path = path.clone();
    let select_path = path.clone();
    rsx! {
        div { class: "guest-tree-node",
            div { class: if selected { "guest-tree-row selected" } else { "guest-tree-row" },
                button { class: "guest-tree-expand", "aria-expanded": expanded.to_string(),
                    "aria-label": format!("{} {}", if expanded { "Collapse" } else { "Expand" }, visible_name(&node.name)),
                    onclick: move |_| {
                        if expanded { state.write().expanded.remove(&inode); }
                        else { navigate.call(open_path.clone()); }
                    },
                    if expanded { "⌄" } else { "›" }
                }
                button { class: "guest-tree-name", title: visible_name(&node.name),
                    onclick: move |_| navigate.call(select_path.clone()), "{visible_name(&node.name)}"
                }
            }
            if expanded {
                div { class: "guest-tree-children",
                    for child in children {
                        Folder { key: "{child.inode}-{child.name}", node: child, parents: path.clone(), state, navigate }
                    }
                }
            }
        }
    }
}
