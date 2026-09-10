use crate::{fields, model};
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn EgressEditor(vm: Value, onclose: EventHandler<()>, onsaved: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut draft = use_signal(|| model::Draft::new(&vm["spec"]["egress"]));
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: "Configure VM egress", onclose,
            form { class: "modal-form",
                onsubmit: move |event| {
                    event.prevent_default();
                    if busy() { return; }
                    let spec = match draft.read().spec(&vm["spec"]) {
                        Ok(spec) => spec,
                        Err(message) => { error.set(message); return; }
                    };
                    let id = text(&vm, "id");
                    busy.set(true);
                    error.set(String::new());
                    spawn(async move {
                        match request("PUT", &format!("/v1/vms/{id}"), Some(spec), &auth.csrf()).await {
                            Ok(_) => onsaved.call(()),
                            Err(message) => error.set(message),
                        }
                        busy.set(false);
                    });
                },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    div { class: "policy-guide",
                        p { "Requires a Firemage-only network. Every destination is denied until you add a rule or tunnel." }
                        Info { title: "Firemage-only egress",
                            "The guest can connect only to its Firemage proxy and tunnel ports at the network gateway. Direct internet access, other guests, and other host ports are blocked. Stop the VM before changing these rules."
                        }
                    }
                    fieldset {
                        legend { "HTTP proxy" }
                        div { class: "radio-group",
                            for (enabled, label) in [(false, "Disabled"), (true, "Enabled")] {
                                label {
                                    input { r#type: "radio", name: "egress-http-enabled", checked: draft.read().http == enabled,
                                        onchange: move |_| draft.write().http = enabled,
                                    }
                                    "{label}"
                                }
                            }
                        }
                    }
                    if draft.read().http {
                        label { class: "field", r#for: "egress-proxy-port",
                            span { "Guest-facing proxy port" }
                            input { id: "egress-proxy-port", r#type: "number", value: "{draft.read().port}",
                                oninput: move |event| draft.write().port = event.value(),
                            }
                        }
                        div { class: "heading compact",
                            h3 { "Allowed HTTP requests"
                                Info { title: "HTTP rules and TLS inspection",
                                    p { "Rules match the exact host, scheme, destination port, method, and path prefix. Hosts do not support wildcards. An empty method list allows standard methods except CONNECT and TRACE. No matching rule means denied." }
                                    p { "HTTPS is always inspected. Install the Firemage CA in the guest and set HTTP_PROXY and HTTPS_PROXY to the proxy URL shown in the Egress tab. The proxy verifies upstream certificates. Clients using certificate pinning need compatible trust settings." }
                                    p { "HTTP targets resolve to public IPs by default. Add explicit CIDRs to reach private services or restrict DNS answers. A private upstream CA can be configured with egress.http.upstream_ca_pem in Full TOML." }
                                }
                            }
                            div { class: "actions wrap",
                                button { r#type: "button", onclick: move |_| draft.write().rules.push(model::openai_rule()), "OpenAI preset" }
                                button { r#type: "button", onclick: move |_| draft.write().rules.push(model::new_rule()), "+ Add HTTP rule" }
                            }
                        }
                        if draft.read().rules.is_empty() {
                            p { class: "small muted", "No HTTP destinations are allowed." }
                        }
                        fields::HttpRules { draft }
                    }
                    div { class: "heading compact egress-section",
                        h3 { "TCP tunnels"
                            Info { title: "TCP port mappings",
                                "A tunnel maps one gateway port to one fixed destination host and port. Point the guest client at gateway:port. Firemage passes raw TCP bytes without HTTP rules or TLS interception. Private targets are allowed; optional destination CIDRs restrict resolved addresses. Each listener port must be unique within this VM."
                            }
                        }
                        button { r#type: "button", onclick: move |_| draft.write().tunnels.push(model::new_tunnel()), "+ Add TCP tunnel" }
                    }
                    fields::Tunnels { draft }
                    crate::upstream::Upstream { draft }
                    if draft.read().tunnels.is_empty() { p { class: "small muted", "No TCP tunnels configured." } }
                }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(),
                        if busy() { "Saving…" } else { "Save egress" }
                    }
                }
            }
        }
    }
}
