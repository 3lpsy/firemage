use crate::mutation::{self, Change, EditorMode};
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_component_value_source::ValueSource;
use firemage_webui_provider_auth::use_auth;
use serde_json::json;

#[component]
pub(crate) fn VariableEditor(
    id: String,
    mode: EditorMode,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
) -> Element {
    let auth = use_auth();
    let is_add = mode == EditorMode::Add;
    let mut rows = use_signal(|| match &mode {
        EditorMode::Add => vec![(String::new(), json!(""))],
        EditorMode::Edit { name, value } => vec![(name.clone(), value.clone())],
    });
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: if is_add { "Add environment variables" } else { "Edit environment variable" },
            onclose: move |_| if !busy() { onclose.call(()); },
            form { class: "modal-form environment-variable-editor", onsubmit: move |event| {
                event.prevent_default();
                if busy() { return; }
                let values = match mutation::validate_rows(&rows.read()) {
                    Ok(values) => values, Err(message) => { error.set(message); return; }
                };
                let change = match &mode {
                    EditorMode::Add => Change::Add(values),
                    EditorMode::Edit { name, value } => Change::Edit {
                        name: name.clone(), previous: value.clone(), replacement: rows.read()[0].clone(),
                    },
                };
                let id = id.clone();
                busy.set(true); error.set(String::new());
                spawn(async move {
                    match mutation::save(&id, change, &auth.csrf()).await {
                        Ok(()) => onsaved.call(()), Err(message) => error.set(message),
                    }
                    busy.set(false);
                });
            },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    fieldset { disabled: busy(), class: "environment-variable-fields",
                        for index in 0..rows.read().len() {
                            fieldset { class: "environment-variable-row", key: "environment-{index}",
                                legend { if is_add { "Variable {index + 1}" } else { "Variable" } }
                                div { class: "environment-variable-grid",
                                    label { class: "field environment-variable-name", r#for: "environment-{index}-name", span { "Name" }
                                        input { id: "environment-{index}-name", value: "{rows.read()[index].0}", placeholder: "SERVICE_TOKEN",
                                            oninput: move |event| rows.write()[index].0 = event.value() }
                                    }
                                    ValueSource { label: "Source", id: "environment-{index}", value: rows.read()[index].1.clone(), plain_text: true,
                                        onchange: move |value| rows.write()[index].1 = value }
                                    if is_add && rows.read().len() > 1 {
                                        button { r#type: "button", class: "danger subtle environment-variable-remove", "aria-label": "Remove environment variable {index + 1}",
                                            onclick: move |_| { rows.write().remove(index); }, "Remove" }
                                    }
                                }
                            }
                        }
                        if is_add {
                            button { r#type: "button", onclick: move |_| rows.write().push((String::new(), json!(""))), "+ Add variable" }
                        }
                    }
                }
                div { class: "actions end",
                    button { r#type: "button", disabled: busy(), onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(),
                        if busy() { "Saving…" } else if is_add { "Add variables" } else { "Save variable" } }
                }
            }
        }
    }
}
