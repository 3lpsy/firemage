use dioxus::prelude::*;
use firemage_webui_component_controls::{Icon, Notice};
use firemage_webui_provider_api::{get, text};
use serde_json::Value;

#[component]
pub fn CatalogSelect(
    kind: &'static str,
    label: String,
    id: String,
    selected: String,
    onchange: EventHandler<String>,
    oncreate: Option<EventHandler<()>>,
    #[props(default)] owner: String,
    #[props(default)] upstream: bool,
) -> Element {
    let mut options = use_resource(move || async move { get(&format!("/v1/egress/{kind}")).await });
    let mut search = use_signal(String::new);
    let filtered = options
        .read()
        .as_ref()
        .and_then(|v| v.as_ref().ok())
        .map(crate::catalog_model::rows)
        .unwrap_or_default()
        .into_iter()
        .filter(|row| owner.is_empty() || text(row, "owner_id") == owner)
        .collect::<Vec<Value>>();
    let loaded = options.read().as_ref().is_some_and(Result::is_ok);
    let found = selected.is_empty()
        || selected == "default"
        || filtered.iter().any(|row| text(row, "id") == selected);
    rsx! {
        div { class: "field",
            div { class: "secret-field-label",
                label { r#for: "{id}", "{label}" }
                if let Some(oncreate) = oncreate {
                    button { r#type: "button", class: "secret-field-action", title: "Create {label}", "aria-label": "Create {label}", onclick: move |_| oncreate.call(()), Icon { name: "plus", size: 15 } }
                }
                button { r#type: "button", class: "secret-field-action", title: "Refresh {label}", "aria-label": "Refresh {label}", onclick: move |_| options.restart(), Icon { name: "refresh", size: 15 } }
            }
            input { r#type: "search", class: "catalog-filter", placeholder: "Search {label}", "aria-label": "Search {label}", value: search(), oninput: move |event| search.set(event.value()) }
            select { id, disabled: !loaded, value: "{selected}", onchange: move |event| onchange.call(event.value()),
                option { value: "", selected: selected.is_empty(), if upstream { "Direct from Firemage" } else { "No outbound access" } }
                if upstream { option { value: "default", selected: selected == "default", "Server default" } }
                for row in filtered.iter().filter(|row| text(row,"id") == selected || text(row,"alias").to_lowercase().contains(&search().to_lowercase())) {
                    option { value: text(row,"id"), selected: text(row,"id") == selected, "{text(row, \"alias\")}" }
                }
                if !found { option { value: "{selected}", selected: true, disabled: true, "Selected resource unavailable" } }
            }
            if let Some(Err(error)) = options.read().as_ref() { Notice { message: error.clone() } }
        }
    }
}
