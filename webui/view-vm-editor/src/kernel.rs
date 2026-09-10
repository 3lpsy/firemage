use dioxus::prelude::*;
use firemage_webui_component_controls::{Info, Notice};
use firemage_webui_provider_api::{get, text};
use std::{collections::HashMap, rc::Rc};

#[component]
pub fn KernelPicker(mut selected: Signal<String>) -> Element {
    let mut rows = use_resource(|| async { get("/v1/kernels").await });
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
    let label = rows
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|value| {
            value
                .as_array()?
                .iter()
                .find(|row| row["name"] == selected())
                .map(|row| {
                    let alias = text(row, "alias");
                    if alias.is_empty() {
                        text(row, "name")
                    } else {
                        format!("{alias} · {}", text(row, "name"))
                    }
                })
        })
        .unwrap_or_else(|| {
            if selected().is_empty() {
                "Choose a kernel".into()
            } else {
                format!("{} (unavailable)", selected())
            }
        });
    rsx! {
        div { class: "kernel-picker",
            label { class: "field", r#for: "vm-kernel",
                span { "Kernel"
                    Info { title: "Choose a kernel", "Select a kernel already in the server's kernel directory. Search by filename or alias. Add uploads and remote downloads from the Kernels page." }
                }
            }
            button { id: "vm-kernel", class: "kernel-trigger", r#type: "button", onmounted: move |event| trigger.set(Some(event.data())), "aria-haspopup": "listbox", "aria-expanded": open(), "aria-controls": "vm-kernel-options",
                onclick: move |_| { open.set(!open()); search.set(String::new()); active.set(None); },
                span { "{label}" } firemage_webui_component_controls::Icon { name: "chevron-down", size: 12 }
            }
            if open() {
                div { class: "kernel-dropdown", onkeydown: move |event| { if event.key() == Key::Escape { event.stop_propagation(); close(); } },
                    input { id: "vm-kernel-search", r#type: "search", role: "combobox", autofocus: true, "aria-label": "Search available kernels", "aria-expanded": "true", "aria-controls": "vm-kernel-options", "aria-autocomplete": "list", "aria-activedescendant": active().map(|index| format!("vm-kernel-option-{index}")), placeholder: "Search filename or alias", value: "{search}",
                        oninput: move |event| { search.set(event.value()); active.set(None); },
                        onkeydown: move |event| {
                            if !matches!(event.key(), Key::ArrowDown | Key::ArrowUp | Key::Enter) { return; }
                            event.prevent_default();
                            let values = rows.read();
                            let Some(Ok(values)) = values.as_ref() else { return; };
                            let names: Vec<_> = values.as_array().into_iter().flatten().filter(|row| {
                                format!("{} {}", text(row, "name"), text(row, "alias")).to_lowercase().contains(&search().to_lowercase())
                            }).map(|row| text(row, "name")).collect();
                            if names.is_empty() { return; }
                            match event.key() {
                                Key::ArrowDown => active.set(Some(active().map_or(0, |index| (index + 1) % names.len()))),
                                Key::ArrowUp => active.set(Some(active().map_or(names.len() - 1, |index| (index + names.len() - 1) % names.len()))),
                                Key::Enter => { selected.set(names[active().unwrap_or(0).min(names.len() - 1)].clone()); close(); },
                                _ => {},
                            }
                        },
                    }
                    match rows.read().as_ref() {
                        Some(Ok(value)) => {
                            let matches: Vec<_> = value.as_array().into_iter().flatten().filter(|row| {
                                format!("{} {}", text(row, "name"), text(row, "alias")).to_lowercase().contains(&search().to_lowercase())
                            }).collect();
                            rsx! {
                                div { id: "vm-kernel-options", role: "listbox", "aria-label": "Available kernels",
                                    for (index, row) in matches.iter().enumerate() {
                                        button { id: "vm-kernel-option-{index}", r#type: "button", onmounted: move |event| { options.write().insert(index, event.data()); }, class: if active() == Some(index) { "kernel-option keyboard-active" } else { "kernel-option" }, role: "option", "aria-selected": row["name"] == selected(),
                                            onclick: { let name = text(row, "name"); move |_| { selected.set(name.clone()); close(); } },
                                            if !text(row, "alias").is_empty() { strong { r#"{text(row, "alias")}"# } }
                                            span { class: "mono", r#"{text(row, "name")}"# }
                                        }
                                    }
                                }
                                if matches.is_empty() { p { class: "small muted", "No matching kernels. Add one from the Kernels page." } }
                            }
                        },
                        Some(Err(message)) => rsx! { Notice { message: message.clone() } },
                        None => rsx! { p { "Loading kernels…" } },
                    }
                    div { class: "actions",
                        button { r#type: "button", onclick: move |_| rows.restart(), "Refresh kernels" }
                        a { href: "#kernels", target: "_blank", rel: "noopener", "Manage kernels" }
                    }
                }
            }
        }
    }
}
