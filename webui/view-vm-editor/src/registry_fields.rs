use crate::registry::RegistryForm;
use dioxus::prelude::*;
use firemage_webui_component_controls::Info;
use firemage_webui_provider_api::{get, text};
use serde_json::Value;

#[component]
pub fn RegistryFields(mut value: Signal<RegistryForm>) -> Element {
    let mut secrets = use_resource(|| async { get("/v1/secrets").await });
    let rows = secrets
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let current = value();
    rsx! {
        fieldset {
            legend {
                "Registry access"
                Info { title: "Registry credentials",
                    "Create credentials in Secrets, then select their names here. Firemage resolves them on the server when pulling the image. Passwords, tokens, and CA contents are never stored in VM configuration or sent into the guest. For a robot account, use its username and store its token as the password secret."
                }
            }
            div { class: "radio-group",
                for (mode, label) in [("anonymous", "Anonymous"), ("basic", "Username and password"), ("bearer", "Bearer token")] {
                    label {
                        input { r#type: "radio", name: "registry-auth", value: mode,
                            checked: current.mode == mode || current.mode.is_empty() && mode == "anonymous",
                            onchange: move |_| value.write().mode = mode.into(),
                        }
                        "{label}"
                    }
                }
            }
            if current.mode == "basic" {
                label { class: "field", r#for: "registry-username", span { "Registry username" }
                    input { id: "registry-username", value: "{current.username}", required: true,
                        oninput: move |event| value.write().username = event.value(),
                    }
                }
                SecretChoice { id: "registry-password", label: "Password secret", value: current.password_secret.clone(), rows: rows.clone(), required: true,
                    onchange: move |name| value.write().password_secret = name,
                }
            } else if current.mode == "bearer" {
                SecretChoice { id: "registry-token", label: "Bearer token secret", value: current.token_secret.clone(), rows: rows.clone(), required: true,
                    onchange: move |name| value.write().token_secret = name,
                }
            }
            label { class: "field", r#for: "registry-realm",
                span { "Trusted token endpoint (optional)"
                    Info { title: "Trusted token endpoint",
                        "Set the exact HTTPS token endpoint when your registry uses a separate authentication service. Firemage sends registry credentials only to the registry or this explicitly trusted endpoint. Do not include URL credentials, query parameters, or a fragment."
                    }
                }
                input { id: "registry-realm", r#type: "url", value: "{current.token_realm}", placeholder: "https://auth.example.com/token",
                    oninput: move |event| value.write().token_realm = event.value(),
                }
            }
            SecretChoice { id: "registry-ca", label: "Custom CA secret (optional)", value: current.ca_secret.clone(), rows,
                onchange: move |name| value.write().ca_secret = name,
            }
            if let Some(Err(error)) = secrets.read().as_ref() {
                p { class: "small", role: "alert", "Unable to load secrets: {error}" }
            }
            div { class: "actions wrap",
                a { class: "small", href: "#secrets", target: "_blank", rel: "noopener", "Manage secrets" }
                button { r#type: "button", class: "small", onclick: move |_| secrets.restart(), "Refresh secrets" }
                Info { title: "Custom registry CA",
                    "Choose a secret containing PEM CA certificates to trust a private certificate authority for registry requests. HTTPS verification stays enabled. This also works for anonymous registries."
                }
            }
        }
    }
}

#[component]
fn SecretChoice(
    id: String,
    label: String,
    value: String,
    rows: Value,
    #[props(default)] required: bool,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "field", r#for: "{id}", span { "{label}" }
            select { id, value: "{value}", required, onchange: move |event| onchange.call(event.value()),
                option { value: "", if required { "Choose a secret" } else { "System trust store" } }
                for row in rows.as_array().into_iter().flatten() {
                    option { value: r#"{text(row, "name")}"#, r#"{text(row, "name")}"# }
                }
                if !value.is_empty() && !rows.as_array().into_iter().flatten().any(|row| row["name"] == value) {
                    option { value: "{value}", "{value} (not available to this account)" }
                }
            }
        }
    }
}
