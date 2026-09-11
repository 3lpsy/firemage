use crate::{catalog_model, drawer::Drawer, policy_page::PolicyEditor, proxy_editor::ProxyEditor};
use dioxus::prelude::*;
use firemage_webui_component_controls::{Empty, Icon, Notice};
use firemage_webui_provider_api::{get, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

pub fn navigate(path: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(&format!("egress/{path}"));
    }
}

#[component]
pub fn EgressCatalog() -> Element {
    let route = firemage_webui_routes::use_egress_path();
    let path = route();
    if path == "policies/new" {
        return rsx! { PolicyEditor { onclose: |_| navigate("policies"), onsaved: |value: Value| navigate(&format!("policies/{}",text(&value,"id"))) } };
    }
    if let Some(id) = path
        .strip_prefix("policies/")
        .and_then(|s| s.strip_suffix("/edit"))
    {
        return rsx! { for id in [id] { EditPage { key: "{id}", id: id.to_owned() } } };
    }
    let proxies = path.starts_with("proxies");
    let selected = path.split('/').nth(1).unwrap_or_default().to_owned();
    // Dioxus applies keys to iterator children, so changing catalogs drops pending requests and local state.
    rsx! { for proxies in [proxies] { Inventory { key: "{proxies}", proxies, selected: selected.clone() } } }
}

#[component]
fn EditPage(id: String) -> Element {
    let row = use_resource(move || {
        let id = id.clone();
        async move { get(&format!("/v1/egress/policies/{id}")).await }
    });
    rsx! {
        match row.read().as_ref() {
            Some(Ok(value)) => rsx! { PolicyEditor { value: value.clone(), onclose: |_| navigate("policies"), onsaved: |value: Value| navigate(&format!("policies/{}",text(&value,"id"))) } },
            Some(Err(message)) => rsx! { Notice { message: message.clone() } },
            None => rsx! { p { "Loading policy…" } },
        }
    }
}

#[component]
fn Inventory(proxies: bool, selected: String) -> Element {
    let auth = use_auth();
    let kind = if proxies { "proxies" } else { "policies" };
    let mut rows = use_resource(move || async move { get(&format!("/v1/egress/{kind}")).await });
    let mut search = use_signal(String::new);
    let mut editing = use_signal(|| None::<Value>);
    let mut refresh = use_signal(|| 0u32);
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            rows.restart();
        }
    });
    rsx! {
        style { {include_str!("catalog.css")} }
        section { class: "egress-catalog-page",
            div { class: "heading",
                div {
                    div { class: "eyebrow", "CONNECTIVITY" }
                    h1 { "Egress" }
                }
                div { class: "actions",
                    button { onclick: move |_| { rows.restart(); refresh += 1; }, "Refresh egress" }
                    if auth.is_admin() { button { class: "primary", onclick: move |_| if proxies { editing.set(Some(Value::Null)); } else { navigate("policies/new"); },
                        if proxies { "Create upstream proxy" } else { "Create policy" }
                    } }
                }
            }
            div { class: "tabs",
                button { class: if !proxies { "active" } else { "" }, onclick: |_| navigate("policies"), "Egress policies" }
                button { class: if proxies { "active" } else { "" }, onclick: |_| navigate("proxies"), "Upstream proxies" }
            }
            div { class: "toolbar", input { id: "egress-search", r#type: "search", placeholder: if proxies { "Search upstream proxies" } else { "Search policies" }, "aria-label": "Search egress", value: search(), oninput: move |event| search.set(event.value()) } }
            div { class: if selected.is_empty() { "egress-catalog" } else { "egress-catalog with-drawer" },
                div {
                    match rows.read().as_ref() {
                        Some(Ok(value)) => {
                            let all = catalog_model::rows(value);
                            let filtered = all.iter().filter(|v|text(v,"alias").to_lowercase().contains(&search().to_lowercase()));
                            rsx! {
                                if all.is_empty() { Empty { title: if proxies { "No upstream proxies" } else { "No egress policies" }, description: if proxies { "Create a proxy to share its route and credentials across policies." } else { "Create a policy, then select it when creating or editing a VM." } } }
                                else {
                                    table { class: "egress-catalog-table",
                                        thead { tr { th { "Alias" } th { if proxies { "Endpoint" } else { "HTTP rules / tunnels" } } if proxies { th { "Policies" } } th { "VMs" } th { "" } } }
                                        tbody {
                                            for row in filtered {
                                                tr { key: "{text(row,\"id\")}", class: if text(row,"id") == selected { "vm-row selected" } else { "vm-row" },
                                                    onclick: { let target = if text(row,"id") == selected { kind.into() } else { format!("{kind}/{}", text(row,"id")) }; move |_| navigate(&target) },
                                                    td { a { class: "table-link", href: format!("#egress/{kind}/{}", text(row,"id")),
                                                        "aria-expanded": (text(row,"id") == selected).to_string(), onclick: move |event| event.stop_propagation(), "{text(row,\"alias\")}" } }
                                                    td { if proxies { "{text(&row[\"proxy\"],\"url\")}" } else { "{row[\"policy\"][\"http\"][\"rules\"].as_array().map_or(0,Vec::len)} / {row[\"policy\"][\"tunnels\"].as_array().map_or(0,Vec::len)}" } }
                                                    if proxies { td { "{catalog_model::usage(row,\"policy\")}" } }
                                                    td { "{catalog_model::usage(row,\"vm\")}" }
                                                    td { class: "vm-row-chevron",
                                                        button { class: "icon-button", title: "Toggle egress details", "aria-label": format!("Toggle {} details", text(row,"alias")),
                                                            "aria-expanded": (text(row,"id") == selected).to_string(),
                                                            onclick: { let target = if text(row,"id") == selected { kind.into() } else { format!("{kind}/{}", text(row,"id")) }; move |event| { event.stop_propagation(); navigate(&target); } },
                                                            Icon { name: "chevron-right", size: 16 }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    p { class: "small muted", "Shared edits apply to every associated VM, including running VMs." }
                                }
                            }
                        },
                        Some(Err(error)) => rsx! { Notice { message: error.clone() } },
                        None => rsx! { p { "Loading egress…" } },
                    }
                }
                for id in [selected.clone()].into_iter().filter(|id| !id.is_empty()) {
                    Drawer { key: "{kind}-{id}-{refresh}", kind, id,
                        onclose: move |_| navigate(kind),
                        onedit: move |value: Value| if proxies { editing.set(Some(value)); } else { navigate(&format!("policies/{}/edit",text(&value,"id"))); },
                        onchanged: move |_| { rows.restart(); refresh += 1; },
                    }
                }
            }
            if let Some(value) = editing() { ProxyEditor { value, onclose: move |_| editing.set(None), onsaved: move |value: Value| { editing.set(None); rows.restart(); refresh += 1; navigate(&format!("proxies/{}",text(&value,"id"))); } } }
        }
    }
}
