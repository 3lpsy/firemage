use crate::{fields, model};
use dioxus::prelude::*;
use firemage_webui_component_controls::{Editor, Info};

pub const SECTIONS: &[(&str, &str)] = &[
    ("identity", "Identity"),
    ("http", "HTTP rules"),
    ("tunnels", "TCP tunnels"),
    ("upstream", "Upstream proxy"),
    ("tls", "Destination TLS"),
];

#[component]
pub fn Section(
    id: &'static str,
    title: &'static str,
    summary: String,
    #[props(default)] open: bool,
    children: Element,
) -> Element {
    rsx! { details { id: "policy-section-{id}", class: "vm-form-section", open,
        summary { strong { "{title}" } span { "{summary}" } }
        div { class: "vm-form-section-body", {children} }
    } }
}
#[component]
pub fn Index() -> Element {
    rsx! { nav { class: "vm-form-index", "aria-label": "Policy configuration sections",
        for &(id,title) in SECTIONS {
            button { r#type: "button", "aria-controls": "policy-section-{id}", onclick: move |_| {
                if let Some(element) = web_sys::window().and_then(|w|w.document()).and_then(|d|d.get_element_by_id(&format!("policy-section-{id}"))) {
                    let _ = element.set_attribute("open", ""); element.scroll_into_view();
                }
            }, "{title}" }
        }
        p { class: "small muted", "All changes save together." }
    } }
}
#[component]
pub fn HttpSection(mut draft: Signal<model::Draft>) -> Element {
    let enabled = draft.read().http;
    let rules = draft.read().rules.len();
    rsx! {
        div { class: "egress-http-section",
            Section { id: "http", title: "HTTP rules", summary: format!("{rules} rules"), open: true,
                Http { draft }
            }
            label { class: "switch-row egress-http-toggle",
                span { if enabled { "Enabled" } else { "Disabled" } }
                input {
                    id: "egress-http-enabled", class: "toggle-switch", r#type: "checkbox", role: "switch",
                    "aria-label": "Enable HTTP proxy", checked: enabled,
                    onchange: move |event| draft.write().http = event.checked(),
                }
            }
        }
    }
}

#[component]
fn Http(mut draft: Signal<model::Draft>) -> Element {
    rsx! {
        if draft.read().http {
            p { class: "small muted", "Every destination is denied until a rule allows it. The VM keeps its existing guest-facing proxy port." }
            div { class: "heading compact", h3 { "Allowed requests"
                Info { title: "HTTP rules", "Match exact hosts, ports, methods and path prefixes. HTTPS is inspected using the Firemage CA. Empty destination CIDRs allow public IPs only." }
            }
                div { class: "actions wrap",
                    button { r#type: "button", onclick: move |_| draft.write().rules.push(model::openai_rule()), "OpenAI preset" }
                    button { r#type: "button", onclick: move |_| draft.write().rules.push(model::new_rule()), "+ Add HTTP rule" }
                }
            }
            fields::HttpRules { draft }
            if draft.read().rules.is_empty() { p { class: "small muted", "No HTTP destinations are allowed." } }
        } else {
            p { class: "small muted", "HTTP proxy is disabled. Rules stay in this draft." }
        }
    }
}
#[component]
pub fn Tunnels(mut draft: Signal<model::Draft>) -> Element {
    rsx! {
        div { class: "heading compact", p { class: "small muted", "Map a gateway port to a fixed TCP destination. Guest clients use gateway:port." }
            button { r#type: "button", onclick: move |_| draft.write().tunnels.push(model::new_tunnel()), "+ Add TCP tunnel" }
        }
        fields::Tunnels { draft }
        if draft.read().tunnels.is_empty() { p { class: "small muted", "No TCP tunnels configured." } }
    }
}
#[component]
pub fn Tls(ca: Signal<String>) -> Element {
    rsx! {
        Editor { label: "Destination CA certificates (optional PEM)", id: "policy-destination-ca", value: ca, rows: 6 }
        p { class: "small muted", "Adds trust for the allowed HTTP destinations. Proxy server trust is configured on the upstream proxy." }
    }
}
