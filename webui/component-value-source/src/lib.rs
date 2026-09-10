//! Literal or write-only secret-reference form control.
use dioxus::prelude::*;
use firemage_webui_component_controls::Icon;
use firemage_webui_provider_api::{get, text};
use serde_json::{Value, json};

#[component]
pub fn ValueSource(
    label: String,
    id: String,
    value: Value,
    onchange: EventHandler<Value>,
    #[props(default)] prefix: bool,
    #[props(default)] plain_text: bool,
) -> Element {
    let mut secrets = use_resource(|| async { get("/v1/secrets").await });
    let is_secret = value.is_object();
    let literal_id = format!("{id}-literal");
    let secret_id = format!("{id}-secret");
    let mode_id = format!("{id}-mode");
    let prefix_id = format!("{id}-prefix");
    let secret_value = value.clone();
    let prefix_value = value.clone();
    rsx! {
        fieldset { class: "value-source",
            legend { "{label}" }
            div { class: "radio-group",
                label { input { r#type: "radio", name: "{mode_id}", checked: !is_secret, onchange: move |_| onchange.call(json!("")) } "Plain value" }
                label { input { r#type: "radio", name: "{mode_id}", checked: is_secret, onchange: move |_| onchange.call(json!({"secret":""})) } "Sensitive secret" }
            }
            if is_secret {
                label { class: "field", r#for: "{secret_id}", span { "Secret name" }
                    select { id: "{secret_id}", value: r#"{text(&value, "secret")}"#,
                        onchange: move |event| { let mut value = secret_value.clone(); value["secret"] = json!(event.value()); onchange.call(value); },
                        option { value: "", "Choose a secret" }
                        if let Some(Ok(rows)) = secrets.read().as_ref() {
                            for row in rows.as_array().into_iter().flatten() {
                                option { value: r#"{text(row, "name")}"#, r#"{text(row, "name")}"# }
                            }
                            if !text(&value, "secret").is_empty() && !rows.as_array().into_iter().flatten().any(|row| row["name"] == value["secret"]) {
                                option { value: r#"{text(&value, "secret")}"#, r#"{text(&value, "secret")} (not available to this account)"# }
                            }
                        }
                    }
                }
                if let Some(Err(error)) = secrets.read().as_ref() {
                    p { class: "small", role: "alert", "Unable to load secrets: {error}" }
                }
                div { class: "actions wrap",
                    a { class: "small", href: "#secrets", target: "_blank", rel: "noopener", "Manage secrets " Icon { name: "external", size: 12 } }
                    button { r#type: "button", class: "small", onclick: move |_| secrets.restart(), "Refresh secrets" }
                }
                if prefix {
                    label { class: "field", r#for: "{prefix_id}", span { "Prefix before secret (optional)" }
                        input { id: "{prefix_id}", value: r#"{text(&value, "prefix")}"#, placeholder: "Bearer ",
                            oninput: move |event| { let mut value = prefix_value.clone(); value["prefix"] = json!(event.value()); onchange.call(value); }
                        }
                    }
                }
            } else {
                if !plain_text { p { class: "small muted", "Plain values are stored in VM configuration. Use a secret to keep credentials write-only." } }
                label { class: "field", r#for: "{literal_id}", span { "Value" }
                    input { id: "{literal_id}", r#type: if plain_text { "text" } else { "password" }, value: value.as_str().unwrap_or_default(),
                        oninput: move |event| onchange.call(json!(event.value())),
                    }
                }
            }
        }
    }
}
