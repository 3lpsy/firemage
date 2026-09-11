use dioxus::prelude::*;
use firemage_webui_component_controls::Icon;
use firemage_webui_provider_api::text;
use serde_json::Value;
use std::{collections::HashMap, rc::Rc};

#[component]
pub fn VmPicker(
    id: String,
    rows: Vec<Value>,
    mut selected: Signal<String>,
    disabled: bool,
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
        if let Some(element) = active().and_then(|index| options.read().get(&index).cloned()) {
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
    let chosen = rows.iter().find(|vm| text(vm, "id") == selected());
    let label = chosen
        .map(crate::model::vm_name)
        .unwrap_or_else(|| "Choose a compatible VM".into());
    let state = chosen.map(|vm| text(vm, "state")).unwrap_or_default();
    let title = chosen.map(crate::model::vm_choice).unwrap_or_default();
    let query = search().to_lowercase();
    let matching: Vec<_> = rows
        .iter()
        .filter(|vm| {
            format!(
                "{} {} {}",
                crate::model::vm_name(vm),
                text(vm, "id"),
                text(vm, "state")
            )
            .to_lowercase()
            .contains(&query)
        })
        .cloned()
        .collect();
    let keyboard_rows = matching.clone();
    let duplicate_names: std::collections::HashSet<_> = rows
        .iter()
        .filter(|vm| {
            rows.iter()
                .filter(|other| crate::model::vm_name(other) == crate::model::vm_name(vm))
                .count()
                > 1
        })
        .map(crate::model::vm_name)
        .collect();
    let list_id = format!("{id}-options");
    let keyboard_id = id.clone();
    rsx! {
        div { class: "kernel-picker snapshot-vm-picker",
            label { class: "field", r#for: id.clone(), span { "Target VM" } }
            button { id: id.clone(), class: "kernel-trigger", r#type: "button", disabled, title,
                onmounted: move |event| trigger.set(Some(event.data())), "aria-haspopup": "listbox", "aria-expanded": open(), "aria-controls": list_id.clone(),
                onclick: move |_| { open.toggle(); search.set(String::new()); active.set(None); },
                span { "{label}" if !state.is_empty() { span { class: "small muted", " · {state}" } } }
                Icon { name: "chevron-down", size: 12 }
            }
            if open() && !disabled {
                div { class: "kernel-dropdown", onkeydown: move |event| { if event.key() == Key::Escape { event.stop_propagation(); close(); } },
                    input { id: "{id}-search", r#type: "search", role: "combobox", autofocus: true,
                        "aria-label": "Search compatible VMs", "aria-expanded": "true", "aria-controls": list_id.clone(), "aria-autocomplete": "list",
                        "aria-activedescendant": active().map(|index| format!("{id}-option-{index}")),
                        placeholder: "Search name or VM ID", value: search(), oninput: move |event| { search.set(event.value()); active.set(None); },
                        onkeydown: move |event| {
                            if !matches!(event.key(), Key::ArrowDown | Key::ArrowUp | Key::Enter) { return; }
                            event.prevent_default();
                            if keyboard_rows.is_empty() { return; }
                            let len = keyboard_rows.len();
                            match event.key() {
                                Key::ArrowDown => active.set(Some(active().map_or(0, |index| (index + 1) % len))),
                                Key::ArrowUp => active.set(Some(active().map_or(len - 1, |index| (index + len - 1) % len))),
                                Key::Enter => { selected.set(text(&keyboard_rows[active().unwrap_or(0).min(len - 1)], "id")); close(); },
                                _ => {},
                            }
                        },
                    }
                    div { id: list_id.clone(), role: "listbox", "aria-label": "Compatible VMs",
                        for (index, vm) in matching.iter().enumerate() {
                            button { id: "{keyboard_id}-option-{index}", r#type: "button", role: "option", "aria-selected": text(vm, "id") == selected(),
                                class: if active() == Some(index) { "kernel-option keyboard-active" } else { "kernel-option" },
                                onmounted: move |event| { options.write().insert(index, event.data()); },
                                onclick: { let value = text(vm, "id"); move |_| { selected.set(value.clone()); close(); } },
                                strong { "{crate::model::vm_name(vm)}" }
                                span { class: "small muted", "{text(vm, \"state\")}" }
                                if duplicate_names.contains(&crate::model::vm_name(vm)) { span { class: "small mono muted", "{text(vm, \"id\")}" } }
                            }
                        }
                    }
                    if matching.is_empty() { p { class: "small muted", "No matching compatible VMs." } }
                }
            }
        }
    }
}
