use crate::model::{Draft, string};
use dioxus::prelude::*;
use serde_json::json;

#[component]
pub fn RuleField(
    draft: Signal<Draft>,
    index: usize,
    name: &'static str,
    label: &'static str,
    #[props(default)] tunnel: bool,
    #[props(default = "text")] kind: &'static str,
    #[props(default)] placeholder: &'static str,
) -> Element {
    let value = {
        let draft = draft.read();
        let rows = if tunnel { &draft.tunnels } else { &draft.rules };
        rows.get(index)
            .map(|row| string(row, name))
            .unwrap_or_default()
    };
    let id = format!(
        "egress-{}-{index}-{name}",
        if tunnel { "tunnel" } else { "rule" }
    );
    rsx! {
        label { class: "field", r#for: "{id}",
            span { "{label}" }
            input { id, r#type: kind, value, placeholder,
                oninput: move |event| {
                    let mut draft = draft.write();
                    let rows = if tunnel { &mut draft.tunnels } else { &mut draft.rules };
                    if let Some(row) = rows.get_mut(index) { row[name] = json!(event.value()); }
                }
            }
        }
    }
}

#[component]
pub fn HttpRules(mut draft: Signal<Draft>) -> Element {
    rsx! {
        for index in 0..draft.read().rules.len() {
            fieldset { class: "egress-rule", key: "http-{index}",
                legend { "HTTP rule {index + 1}" }
                div { class: "heading compact",
                    div { class: "radio-group",
                        for scheme in ["https", "http"] {
                            label {
                                input { r#type: "radio", name: "egress-rule-{index}-scheme",
                                    checked: draft.read().rules[index]["scheme"] == scheme,
                                    onchange: move |_| {
                                        let mut draft = draft.write();
                                        let rule = &mut draft.rules[index];
                                        let previous = string(rule, "port");
                                        if previous == "443" || previous == "80" {
                                            rule["port"] = json!(if scheme == "https" { 443 } else { 80 });
                                        }
                                        rule["scheme"] = json!(scheme);
                                    }
                                }
                                if scheme == "https" { "HTTPS, inspected" } else { "HTTP" }
                            }
                        }
                    }
                    button { r#type: "button", class: "danger subtle", "aria-label": "Remove HTTP rule {index + 1}",
                        onclick: move |_| { draft.write().rules.remove(index); }, "Remove"
                    }
                }
                div { class: "form-grid egress-rule-grid",
                    RuleField { draft, index, name: "host", label: "Exact host", placeholder: "api.example.com" }
                    RuleField { draft, index, name: "port", label: "Destination port", kind: "number" }
                    RuleField { draft, index, name: "methods", label: "Methods, comma separated", placeholder: "All standard methods" }
                    RuleField { draft, index, name: "path_prefix", label: "Path prefix", placeholder: "/" }
                }
                RuleField { draft, index, name: "allowed_ips", label: "Allowed destination CIDRs (optional)", placeholder: "Public IPs only when empty" }
                crate::injection::Injection { draft, index }
            }
        }
    }
}

#[component]
pub fn Tunnels(mut draft: Signal<Draft>) -> Element {
    rsx! {
        for index in 0..draft.read().tunnels.len() {
            fieldset { class: "egress-rule", key: "tunnel-{index}",
                legend { "TCP tunnel {index + 1}" }
                div { class: "heading compact",
                    span { class: "small muted", "Fixed upstream destination" }
                    button { r#type: "button", class: "danger subtle", "aria-label": "Remove TCP tunnel {index + 1}",
                        onclick: move |_| { draft.write().tunnels.remove(index); }, "Remove"
                    }
                }
                div { class: "form-grid egress-rule-grid",
                    RuleField { draft, index, tunnel: true, name: "name", label: "Tunnel name", placeholder: "database" }
                    RuleField { draft, index, tunnel: true, name: "listen_port", label: "Guest-facing port", kind: "number" }
                    RuleField { draft, index, tunnel: true, name: "target_host", label: "Destination host", placeholder: "db.example.com" }
                    RuleField { draft, index, tunnel: true, name: "target_port", label: "Destination port", kind: "number" }
                }
                RuleField { draft, index, tunnel: true, name: "allowed_ips", label: "Destination CIDRs (optional)", placeholder: "Restrict resolved addresses" }
            }
        }
    }
}
