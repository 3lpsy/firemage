use crate::AttachmentForm;
use dioxus::prelude::*;
use firemage_wire::FileAsset;
use std::{collections::HashMap, rc::Rc};

#[component]
pub fn AssetPicker(
    mut value: Signal<Vec<AttachmentForm>>,
    index: usize,
    rows: Vec<FileAsset>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut search = use_signal(String::new);
    let mut active = use_signal(|| None::<usize>);
    let mut trigger = use_signal(|| None::<Rc<MountedData>>);
    let mut options = use_signal(HashMap::<usize, Rc<MountedData>>::new);
    let mut close = move || {
        open.set(false);
        if let Some(element) = trigger() {
            spawn(async move {
                let _ = element.set_focus(true).await;
            });
        }
    };
    use_effect(move || {
        let element = active().and_then(|index| options.read().get(&index).cloned());
        if let Some(element) = element {
            spawn(async move {
                let _ = element
                    .scroll_to_with_options(ScrollToOptions {
                        behavior: ScrollBehavior::Instant,
                        vertical: ScrollLogicalPosition::Nearest,
                        horizontal: ScrollLogicalPosition::Nearest,
                    })
                    .await;
            });
        }
    });
    let selected = value.read()[index].asset_id.clone();
    let label = rows
        .iter()
        .find(|row| row.id == selected)
        .map(|row| format!("{} · {}", row.alias, row.filename))
        .unwrap_or_else(|| {
            if selected.is_empty() {
                "Choose an asset".into()
            } else {
                "Asset unavailable".into()
            }
        });
    let matching: Vec<_> = rows
        .into_iter()
        .filter(|row| {
            format!("{} {}", row.alias, row.filename)
                .to_lowercase()
                .contains(&search().to_lowercase())
        })
        .collect();
    let keyboard_rows = matching.clone();
    rsx! {
        div { class: "kernel-picker",
            label { class: "field", r#for: "vm-asset-{index}", span { "Asset" } }
            button { id: "vm-asset-{index}", class: "kernel-trigger", r#type: "button", onmounted: move |event| trigger.set(Some(event.data())), "aria-haspopup": "listbox", "aria-expanded": open(), "aria-controls": "vm-asset-options-{index}",
                onclick: move |_| { open.set(!open()); search.set(String::new()); active.set(None); },
                span { "{label}" } firemage_webui_component_controls::Icon { name: "chevron-down", size: 12 }
            }
            if open() {
                div { class: "kernel-dropdown", onkeydown: move |event| { if event.key() == Key::Escape { event.stop_propagation(); close(); } },
                    input { id: "vm-asset-search-{index}", r#type: "search", role: "combobox", autofocus: true, "aria-label": "Search available assets", "aria-expanded": "true", "aria-controls": "vm-asset-options-{index}", "aria-autocomplete": "list", "aria-activedescendant": active().map(|row| format!("vm-asset-option-{index}-{row}")), placeholder: "Search alias or filename", value: "{search}",
                        oninput: move |event| { search.set(event.value()); active.set(None); },
                        onkeydown: move |event| {
                            if !matches!(event.key(), Key::ArrowDown | Key::ArrowUp | Key::Enter) { return; }
                            event.prevent_default();
                            if keyboard_rows.is_empty() { return; }
                            let len = keyboard_rows.len();
                            match event.key() {
                                Key::ArrowDown => active.set(Some(active().map_or(0, |row| (row + 1) % len))),
                                Key::ArrowUp => active.set(Some(active().map_or(len - 1, |row| (row + len - 1) % len))),
                                Key::Enter => { value.write()[index].asset_id = keyboard_rows[active().unwrap_or(0).min(len - 1)].id.clone(); close(); },
                                _ => {},
                            }
                        },
                    }
                    div { id: "vm-asset-options-{index}", role: "listbox", "aria-label": "Available assets",
                        for (row_index, row) in matching.iter().enumerate() {
                            button { id: "vm-asset-option-{index}-{row_index}", r#type: "button", onmounted: move |event| { options.write().insert(row_index, event.data()); }, class: if active() == Some(row_index) { "kernel-option keyboard-active" } else { "kernel-option" }, role: "option", "aria-selected": row.id == selected,
                                onclick: { let id = row.id.clone(); move |_| { value.write()[index].asset_id = id.clone(); close(); } },
                                strong { "{row.alias}" }
                                span { class: "small muted", "{row.filename}" }
                            }
                        }
                    }
                    if matching.is_empty() { p { class: "small muted", "No matching assets. Upload files from Assets in the sidebar." } }
                }
            }
        }
    }
}
