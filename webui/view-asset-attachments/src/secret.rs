use crate::AttachmentForm;
use dioxus::prelude::*;
use firemage_webui_component_controls::{Notice, SecretFieldLabel};
use firemage_webui_provider_api::{get, text};

#[component]
pub fn SecretPicker(mut value: Signal<Vec<AttachmentForm>>, index: usize) -> Element {
    let mut rows = use_resource(|| async { get("/v1/secrets").await });
    rsx! {
        div { class: "secret-attachment-picker",
            div { class: "field",
                SecretFieldLabel { id: "vm-secret-file-{index}", label: "Secret name", onrefresh: move |_| rows.restart() }
                input { id: "vm-secret-file-{index}", list: "vm-secret-file-options-{index}", required: true,
                    placeholder: "Search secret names", autocomplete: "off",
                    value: value.read()[index].secret.clone().unwrap_or_default(),
                    oninput: move |event| value.write()[index].secret = Some(event.value()),
                }
                datalist { id: "vm-secret-file-options-{index}",
                    if let Some(Ok(rows)) = rows.read().as_ref() {
                        for row in rows.as_array().into_iter().flatten() { option { value: text(row, "name") } }
                    }
                }
            }
            if let Some(Err(error)) = rows.read().as_ref() { Notice { message: error.clone() } }
        }
    }
}
