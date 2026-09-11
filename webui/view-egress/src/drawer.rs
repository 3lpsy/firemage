use dioxus::prelude::*;
use firemage_webui_component_controls::{Confirm, Icon, Notice, Status};
use firemage_webui_provider_api::{get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn Drawer(
    kind: &'static str,
    id: String,
    onclose: EventHandler<()>,
    onedit: EventHandler<Value>,
    onchanged: EventHandler<()>,
) -> Element {
    let auth = use_auth();
    let mut detail = use_resource(move || {
        let id = id.clone();
        async move { get(&format!("/v1/egress/{kind}/{id}")).await }
    });
    let mut deleting = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(String::new);
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            detail.restart();
        }
    });
    let title = detail
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|value| text(value, "alias"))
        .unwrap_or_else(|| {
            if kind == "proxies" {
                "Upstream proxy".into()
            } else {
                "Egress policy".into()
            }
        });
    rsx! {
        aside { class: "egress-catalog-drawer", "aria-label": "Egress details",
            div { class: "heading compact", h2 { "{title}" }
                button { class: "icon-button", title: "Close egress details", "aria-label": "Close egress details", onclick: move |_| onclose.call(()), Icon { name: "close" } }
            }
            match detail.read().as_ref() {
                Some(Ok(value)) => {
                    let value = value.clone();
                    let count = crate::catalog_model::usage(&value, if kind == "proxies" { "policy" } else { "vm" });
                    let edit_value = value.clone();
                    let delete_value = value.clone();
                    rsx! {
                        Notice { message: error() }
                        if auth.is_admin() { button { class: "primary", onclick: move |_| onedit.call(edit_value.clone()), if kind == "proxies" { "Edit proxy" } else { "Edit policy" } } }
                        dl { class: "egress-facts",
                            dt { "Alias" } dd { "{text(&value,\"alias\")}" }
                            dt { "Revision" } dd { "{value[\"revision\"]}" }
                            dt { "Owner" } dd { "{text(&value,\"owner_id\")}" }
                        }
                        if kind == "proxies" { ProxyDetails { value: value.clone() } }
                        else { PolicyDetails { key: r#"policy-details-{value["revision"]}"#, value: value.clone() } }
                        h3 { "Associated VMs ({crate::catalog_model::usage(&value,\"vm\")})" }
                        for vm in value["vms"].as_array().into_iter().flatten() {
                            div { class: "egress-reference-row",
                                a { href: "#vms/{text(vm,\"id\")}", "{crate::catalog_model::vm_label(vm)}" }
                                Status { value: text(vm,"state") }
                                if let Some(message) = vm["error"].as_str() { span { class: "small", role: "alert", "{message}" } }
                            }
                        }
                        if crate::catalog_model::usage(&value,"vm") == 0 { p { class: "small muted", "No VMs use this resource." } }
                        if auth.is_admin() {
                            div { class: "egress-delete",
                                button { class: "danger subtle", disabled: count > 0 || busy(), onclick: move |_| deleting.set(true), if kind == "proxies" { "Delete proxy" } else { "Delete policy" } }
                                if count > 0 { p { class: "small muted", if kind == "proxies" { "Used by {count} policies. Reassign those policies before deleting." } else { "Used by {count} VMs, including inactive VMs. Reassign them before deleting." } } }
                            }
                        }
                        if deleting() { Confirm { title: "Delete egress resource", description: format!("Delete {}? This cannot be undone.",text(&value,"alias")), label: "Delete",
                            onclose: move |_| deleting.set(false), onconfirm: move |_| {
                                if busy() { return; } busy.set(true); deleting.set(false);
                                let path = format!("/v1/egress/{kind}/{}",text(&delete_value,"id"));
                                spawn(async move { match request("DELETE",&path,None,&auth.csrf()).await { Ok(_) => { onchanged.call(()); onclose.call(()); }, Err(message) => error.set(message) }; busy.set(false); });
                            }
                        } }
                    }
                },
                Some(Err(message)) => rsx! { Notice { message: message.clone() } },
                None => rsx! { p { "Loading details…" } },
            }
        }
    }
}

#[component]
fn PolicyDetails(value: Value) -> Element {
    let policy = &value["policy"];
    let upstream_id = text(&value, "upstream_proxy_id");
    let upstream = use_resource(move || {
        let id = upstream_id.clone();
        async move {
            if id.is_empty() {
                Ok(Value::Null)
            } else {
                get(&format!("/v1/egress/proxies/{id}")).await
            }
        }
    });
    rsx! {
        dl { class: "egress-facts",
            dt { "Upstream" } dd {
                if value["upstream_proxy_id"].is_string() {
                    a { href: "#egress/proxies/{text(&value,\"upstream_proxy_id\")}", "{upstream.read().as_ref().and_then(|v|v.as_ref().ok()).map(|v|text(v,\"alias\")).unwrap_or_else(||\"View proxy\".into())}" }
                } else if policy["inherit_upstream"].as_bool().unwrap_or(false) { "Server default" } else { "Direct" }
            }
            dt { "HTTP proxy" } dd { if policy["http"].is_object() { "Enabled" } else { "Disabled" } }
        }
        h3 { "HTTP rules" }
        for rule in policy["http"]["rules"].as_array().into_iter().flatten() {
            div { class: "egress-rule-summary",
                strong { "{text(rule,\"host\")}:{rule[\"port\"]}" }
                p { class: "small", "{text(rule,\"scheme\")} · {crate::model::string(rule,\"methods\")} · {text(rule,\"path_prefix\")}" }
                for (name,source) in rule["headers"].as_object().into_iter().flatten() {
                    p { class: "small muted", "{name}: " if source["secret"].is_string() { "secret {text(source,\"secret\")}" } else { "configured value" } }
                }
                if rule["signing"].is_object() { p { class: "small muted", "Signing: {text(&rule[\"signing\"],\"kind\")}" } }
            }
        }
        p { class: "small muted", "All unmatched destinations are denied." }
        h3 { "TCP tunnels" }
        for tunnel in policy["tunnels"].as_array().into_iter().flatten() { p { class: "small", "{text(tunnel,\"name\")}: {tunnel[\"listen_port\"]} → {text(tunnel,\"target_host\")}:{tunnel[\"target_port\"]}" } }
        if policy["tunnels"].as_array().is_none_or(Vec::is_empty) { p { class: "small muted", "None" } }
    }
}

#[component]
fn ProxyDetails(value: Value) -> Element {
    rsx! {
        dl { class: "egress-facts",
            dt { "Endpoint" } dd { "{text(&value[\"proxy\"],\"url\")}" }
            dt { "Username" } dd { if value["proxy"]["username"]["secret"].is_string() { "Secret: {text(&value[\"proxy\"][\"username\"],\"secret\")}" } else { "{text(&value[\"proxy\"],\"username\")}" } }
            dt { "Password" } dd { if value["proxy"]["password"]["secret"].is_string() { "Secret: {text(&value[\"proxy\"][\"password\"],\"secret\")}" } else if value["proxy"]["password"].is_null() { "Not configured" } else { "Configured" } }
            dt { "Custom CA" } dd { if value["ca_secret"].is_string() { "Secret: {text(&value,\"ca_secret\")}" } else if value["proxy"]["ca_pem"].is_string() { "Configured certificates" } else { "System trust" } }
        }
        h3 { "Policies using this proxy" }
        for policy in value["policies"].as_array().into_iter().flatten() {
            div { class: "egress-reference-row", a { href: "#egress/policies/{text(policy,\"id\")}", "{text(policy,\"alias\")}" } }
        }
        if crate::catalog_model::usage(&value,"policy") == 0 { p { class: "small muted", "No policies use this proxy." } }
    }
}
