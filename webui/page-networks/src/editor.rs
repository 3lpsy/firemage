use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{encode, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn NetworkEditor(
    initial: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
) -> Element {
    let auth = use_auth();
    let original = initial["name"].as_str().map(str::to_owned);
    let editing = original.is_some();
    let name = use_signal(|| text(&initial, "name"));
    let mut subnet = use_signal(|| {
        initial["subnet"]
            .as_str()
            .unwrap_or("172.30.0.0/24")
            .to_owned()
    });
    let mut gateway =
        use_signal(|| crate::gateway::Gateway::new(initial["gateway"].as_str(), &subnet()));
    let mut policy = use_signal(|| {
        initial["policy"]["mode"]
            .as_str()
            .unwrap_or("isolated")
            .to_owned()
    });
    let address = use_signal(|| text(&initial["policy"], "address"));
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: if editing { "Edit network" } else { "Create network" }, onclose,
            form {
                class: "modal-form",
                onsubmit: move |e| {
                    e.prevent_default();
                    if busy() {
                        return;
                    }
                    let mut rule = json!({ "mode" : policy() });
                    if policy() == "host-only" {
                        rule["address"] = json!(address());
                    }
                    let body = json!(
                        { "name" : name(), "subnet" : subnet(), "gateway" : gateway().value, "policy" :
                        rule }
                    );
                    let path = original
                        .as_ref()
                        .map(|s| format!("/v1/networks/{}", encode(s)))
                        .unwrap_or("/v1/networks".into());
                    busy.set(true);
                    spawn(async move {
                        match request(
                                if editing { "PUT" } else { "POST" },
                                &path,
                                Some(body),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => onsaved.call(()),
                            Err(e) => error.set(e),
                        }
                        busy.set(false);
                    });
                },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    Field {
                        label: "Network name",
                        id: "network-name",
                        value: name,
                        required: true,
                        disabled: editing,
                    }
                    div { class: "form-grid",
                        label { class: "field", span { "IPv4 subnet" }
                            input {
                                id: "network-subnet", value: "{subnet}", required: true,
                                oninput: move |event| {
                                    gateway.write().update_subnet(&event.value());
                                    subnet.set(event.value());
                                },
                            }
                        }
                        label { class: "field", span { "Gateway" }
                            input {
                                id: "network-gateway", value: gateway().value, required: true,
                                oninput: move |event| gateway.write().edit(event.value()),
                            }
                        }
                    }
                    fieldset {
                        legend {
                            "Access policy"
                            Info { title: "Network access policies",
                                "Firemage only permits this VM to reach its assigned HTTP proxy and TCP tunnel ports on the gateway. Direct upstream access, other host ports, and other guests are blocked. Configure the destinations in each VM’s Egress tab. Specific host IP permits every port at one host address."
                            }
                        }
                        div { class: "radio-stack",
                            for (value, label, description) in [
                                ("isolated", "Isolated", "An interface without external access."),
                                ("firemage-only", "Firemage only", "Per-VM HTTP rules and TCP tunnels. Direct access is blocked."),
                                ("host-only", "Specific host IP", "Access to one explicit host address."),
                                ("unrestricted", "Unrestricted", "Routed host and external network access."),
                            ]
                            {
                                label {
                                    input {
                                        r#type: "radio",
                                        name: "network-policy",
                                        checked: policy() == value,
                                        onchange: move |_| policy.set(value.into()),
                                    }
                                    span {
                                        strong { "{label}" }
                                        small { "{description}" }
                                    }
                                }
                            }
                        }
                    }
                    if policy() == "host-only" {
                        Field {
                            label: "Allowed host IPv4 address",
                            id: "network-allowed",
                            value: address,
                            required: true,
                            placeholder: "192.0.2.10",
                        }
                    }
                }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button {
                        class: "primary",
                        r#type: "submit",
                        disabled: busy(),
                        "Save network"
                    }
                }
            }
        }
    }
}
