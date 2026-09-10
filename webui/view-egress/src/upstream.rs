use crate::model::Draft;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_component_value_source::ValueSource;
use serde_json::json;

#[component]
pub fn Upstream(mut draft: Signal<Draft>) -> Element {
    let mode = draft.read().upstream["url"]
        .as_str()
        .map(|url| {
            if url.starts_with("socks5://") {
                "socks5"
            } else {
                "http"
            }
        })
        .unwrap_or(if draft.read().inherit_upstream {
            "default"
        } else {
            "direct"
        });
    rsx! {
        fieldset { class: "egress-section",
            legend { "Upstream connection"
                Info { title: "Upstream proxy routing",
                    p { "The VM connects to Firemage, which enforces its policy before connecting through the upstream proxy to the destination. HTTP CONNECT and SOCKS5 proxies can carry HTTP requests and raw TCP tunnels." }
                    p { "If the upstream proxy fails, requests fail. Firemage never falls back to a direct connection. Server default uses the upstream configured in server TOML, or connects directly when no server upstream is configured." }
                    p { "Credentials are sent only to the configured proxy. HTTPS proxy certificates are verified; a private proxy CA can be configured with egress.upstream.ca_pem in Full TOML." }
                }
            }
            div { class: "radio-group",
                for (choice, label, url) in [("default", "Server default", ""), ("direct", "Direct", ""), ("http", "HTTP proxy", "http://"), ("socks5", "SOCKS5 proxy", "socks5://")] {
                    label {
                        input { r#type: "radio", name: "egress-upstream-mode", checked: mode == choice,
                            onchange: move |_| { let mut draft = draft.write(); draft.inherit_upstream = choice != "direct"; draft.upstream = if choice == "default" || choice == "direct" { serde_json::Value::Null } else { json!({"url":url}) }; },
                        }
                        "{label}"
                    }
                }
            }
            if mode == "http" || mode == "socks5" {
                label { class: "field", r#for: "egress-upstream-url", span { "Proxy URL" }
                    input { id: "egress-upstream-url", value: draft.read().upstream["url"].as_str().unwrap_or_default(), placeholder: "https://proxy.example.com:3128",
                        oninput: move |event| draft.write().upstream["url"] = json!(event.value()),
                    }
                }
                p { class: "small muted", "VM → Firemage → upstream proxy → allowed destination" }
                ValueSource { label: "Proxy username (optional)", id: "egress-upstream-username", value: draft.read().upstream["username"].clone(), plain_text: true,
                    onchange: move |value| draft.write().upstream["username"] = value,
                }
                ValueSource { label: "Proxy password (optional)", id: "egress-upstream-password", value: draft.read().upstream["password"].clone(),
                    onchange: move |value| draft.write().upstream["password"] = value,
                }
            }
        }
    }
}
