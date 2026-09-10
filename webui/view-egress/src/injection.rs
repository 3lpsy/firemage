use crate::model::Draft;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_component_value_source::ValueSource;
use serde_json::{Value, json};

#[component]
pub fn Injection(mut draft: Signal<Draft>, index: usize) -> Element {
    let rows = draft.read().rules[index]["_headers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    rsx! {
        details { class: "egress-credentials",
            summary { "Credentials and request signing" }
            p { class: "small muted", "Added after policy checks. Secret values remain outside the guest." }
            for (header_index, header) in rows.iter().enumerate() {
                div { class: "egress-rule",
                    div { class: "heading compact",
                        label { class: "field", r#for: "rule-{index}-header-{header_index}-name", span { "Header name" }
                            input { id: "rule-{index}-header-{header_index}-name", value: header["name"].as_str().unwrap_or_default(), placeholder: "Authorization",
                                oninput: move |event| draft.write().rules[index]["_headers"][header_index]["name"] = json!(event.value()),
                            }
                        }
                        button { r#type: "button", class: "danger subtle", "aria-label": "Remove header {header_index + 1}",
                            onclick: move |_| { draft.write().rules[index]["_headers"].as_array_mut().unwrap().remove(header_index); }, "Remove"
                        }
                    }
                    ValueSource { label: "Header value", id: "rule-{index}-header-{header_index}", value: header["value"].clone(), prefix: true,
                        onchange: move |value| draft.write().rules[index]["_headers"][header_index]["value"] = value,
                    }
                }
            }
            button { r#type: "button", onclick: move |_| {
                let mut draft = draft.write();
                if !draft.rules[index]["_headers"].is_array() { draft.rules[index]["_headers"] = json!([]); }
                draft.rules[index]["_headers"].as_array_mut().unwrap().push(json!({"name":"Authorization","value":{"secret":"","prefix":"Bearer "}}));
            }, "+ Add injected header" }
            Signing { draft, index }
        }
    }
}

#[component]
fn Signing(mut draft: Signal<Draft>, index: usize) -> Element {
    let signing = draft.read().rules[index]["signing"].clone();
    let kind = signing["kind"].as_str().unwrap_or("none");
    rsx! {
        fieldset { class: "egress-section",
            legend { "Request signing"
                Info { title: "Signing upstream requests",
                    p { "AWS SigV4 signs requests for the selected region and service using your named credentials. HMAC SHA-256 adds a signature to the configured header. Signing happens after authorization and header injection." }
                    p { "Choose the format expected by the upstream service. HMAC signs METHOD, path with query, Unix timestamp, and the lowercase body SHA-256 joined with newlines. The timestamp is sent in x-firemage-date." }
                }
            }
            div { class: "radio-group",
                for (choice, label) in [("none", "None"), ("aws_sigv4", "AWS SigV4"), ("hmac_sha256", "HMAC SHA-256")] {
                    label { input { r#type: "radio", name: "rule-{index}-signing", checked: kind == choice,
                        onchange: move |_| draft.write().rules[index]["signing"] = match choice {
                            "aws_sigv4" => json!({"kind":choice,"region":"","service":"","access_key":{"secret":""},"secret_key":{"secret":""}}),
                            "hmac_sha256" => json!({"kind":choice,"key":{"secret":""},"header":"X-Signature","prefix":""}),
                            _ => Value::Null,
                        },
                    } "{label}" }
                }
            }
            if kind == "aws_sigv4" {
                div { class: "form-grid",
                    SigningField { draft, index, name: "region", label: "AWS region" }
                    SigningField { draft, index, name: "service", label: "AWS service" }
                }
                for (name, label) in [("access_key", "Access key"), ("secret_key", "Secret key"), ("session_token", "Session token (optional)")] {
                    ValueSource { label, id: "rule-{index}-signing-{name}", value: signing[name].clone(),
                        onchange: move |value| draft.write().rules[index]["signing"][name] = value,
                    }
                }
            }
            if kind == "hmac_sha256" {
                SigningField { draft, index, name: "header", label: "Signature header" }
                SigningField { draft, index, name: "prefix", label: "Signature prefix (optional)" }
                ValueSource { label: "HMAC key", id: "rule-{index}-signing-key", value: signing["key"].clone(),
                    onchange: move |value| draft.write().rules[index]["signing"]["key"] = value,
                }
            }
        }
    }
}
#[component]
fn SigningField(
    mut draft: Signal<Draft>,
    index: usize,
    name: &'static str,
    label: &'static str,
) -> Element {
    rsx! {
        label { class: "field", r#for: "rule-{index}-signing-{name}", span { "{label}" }
            input { id: "rule-{index}-signing-{name}", value: draft.read().rules[index]["signing"][name].as_str().unwrap_or_default(),
                oninput: move |event| draft.write().rules[index]["signing"][name] = json!(event.value()),
            }
        }
    }
}
