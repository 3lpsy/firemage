use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_component_value_source::ValueSource;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn EnvironmentEditor(
    vm: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
) -> Element {
    let auth = use_auth();
    let mut rows = use_signal(|| {
        vm["spec"]["environment"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>()
    });
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: "Configure guest environment", onclose,
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() { return; }
                let mut environment = serde_json::Map::new();
                for (name, value) in rows.read().iter() {
                    if name.is_empty() || !name.bytes().enumerate().all(|(index, byte)| byte == b'_' || byte.is_ascii_alphabetic() || index > 0 && byte.is_ascii_digit()) {
                        error.set("Environment names must start with a letter or underscore and contain only letters, digits, and underscores.".into()); return;
                    }
                    if value.is_object() && value["secret"].as_str().is_none_or(str::is_empty) {
                        error.set(format!("Choose a secret for {name}.")); return;
                    }
                    if environment.insert(name.clone(), value.clone()).is_some() {
                        error.set(format!("Duplicate environment variable: {name}.")); return;
                    }
                }
                let mut spec = vm["spec"].clone();
                spec["environment"] = Value::Object(environment);
                let path = format!("/v1/vms/{}", text(&vm, "id"));
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match request("PUT", &path, Some(spec), &auth.csrf()).await {
                        Ok(_) => onsaved.call(()), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    p { class: "small muted", "Sensitive values are stored as secret references. The guest receives their resolved values at boot." }
                    for index in 0..rows.read().len() {
                        fieldset { class: "egress-rule", key: "environment-{index}",
                            legend { "Variable {index + 1}" }
                            div { class: "heading compact",
                                label { class: "field", r#for: "environment-{index}-name", span { "Name" }
                                    input { id: "environment-{index}-name", value: "{rows.read()[index].0}", placeholder: "SERVICE_TOKEN",
                                        oninput: move |event| rows.write()[index].0 = event.value(),
                                    }
                                }
                                button { r#type: "button", class: "danger subtle", "aria-label": "Remove environment variable {index + 1}", onclick: move |_| { rows.write().remove(index); }, "Remove" }
                            }
                            ValueSource { label: "Source", id: "environment-{index}", value: rows.read()[index].1.clone(), plain_text: true,
                                onchange: move |value| rows.write()[index].1 = value,
                            }
                        }
                    }
                    button { r#type: "button", onclick: move |_| rows.write().push((String::new(), json!(""))), "+ Add variable" }
                }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), "Save environment" }
                }
            }
        }
    }
}
