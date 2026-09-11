use dioxus::prelude::*;
use firemage_webui_provider_api::{encode, get};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Directory {
    pub entries: Vec<Entry>,
    pub max_file_bytes: u64,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Entry {
    pub inode: u64,
    pub name: String,
    pub kind: String,
    pub size_bytes: Option<u64>,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Crumb {
    pub inode: u64,
    pub name: String,
}
#[derive(Clone)]
pub struct BrowserState {
    pub path: Vec<Crumb>,
    pub expanded: BTreeSet<u64>,
    pub directories: BTreeMap<u64, Directory>,
    pub errors: BTreeMap<u64, String>,
    pub loading: BTreeSet<u64>,
}
impl Default for BrowserState {
    fn default() -> Self {
        Self {
            path: vec![Crumb {
                inode: 2,
                name: "/".into(),
            }],
            expanded: BTreeSet::from([2]),
            directories: BTreeMap::new(),
            errors: BTreeMap::new(),
            loading: BTreeSet::new(),
        }
    }
}

pub async fn load(id: String, inode: u64, mut state: Signal<BrowserState>) {
    if state.peek().loading.contains(&inode) || state.peek().directories.contains_key(&inode) {
        return;
    }
    state.write().loading.insert(inode);
    let result = get(&format!("/v1/vms/{}/directory?inode={inode}", encode(&id)))
        .await
        .and_then(|value| serde_json::from_value::<Directory>(value).map_err(|e| e.to_string()));
    let mut current = state.write();
    current.loading.remove(&inode);
    match result {
        Ok(mut directory) => {
            directory
                .entries
                .retain(|entry| !matches!(entry.name.as_str(), "." | ".."));
            directory.entries.sort_by(|a, b| {
                (a.kind != "directory", &a.name).cmp(&(b.kind != "directory", &b.name))
            });
            current.errors.remove(&inode);
            current.directories.insert(inode, directory);
        }
        Err(error) => {
            current.errors.insert(inode, error);
        }
    }
}

pub fn visible_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                c.escape_default().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}
pub fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
