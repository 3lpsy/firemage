//! Reusable asset selection and per-VM destination fields.
mod form;
mod picker;
use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::get;
use firemage_wire::FileAsset;
pub use form::{AttachmentForm, attachments, from_spec};

#[component]
pub fn Attachments(mut value: Signal<Vec<AttachmentForm>>) -> Element {
    let mut rows = use_resource(|| async {
        let value = get("/v1/assets").await?;
        serde_json::from_value::<Vec<FileAsset>>(value).map_err(|e| e.to_string())
    });
    rsx! {
        section { class: "vm-attachments",
            div { class: "heading compact",
                h3 { "VM assets" }
                button { r#type: "button", onclick: move |_| rows.restart(), "Refresh assets" }
            }
            match rows.read().as_ref() {
                Some(Err(message)) => rsx! { Notice { message: message.clone() } },
                None => rsx! { p { "Loading assets…" } },
                _ => rsx! {},
            }
            for index in 0..value.read().len() {
                div { class: "attachment-row",
                    div { class: "attachment-fields",
                        picker::AssetPicker { value, index, rows: rows.read().as_ref().and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default() }
                        label { class: "field", r#for: "asset-destination-{index}", span { "Destination in VM" }
                            input { id: "asset-destination-{index}", required: true, placeholder: "/etc/app/config.json", value: "{value.read()[index].destination}", oninput: move |e| value.write()[index].destination = e.value() }
                        }
                        button { r#type: "button", class: "danger subtle", "aria-label": "Remove asset {index + 1}", onclick: move |_| { value.write().remove(index); }, "Remove" }
                    }
                    details { summary { "File ownership and permissions" }
                        div { class: "form-grid",
                            label { class: "field", r#for: "asset-uid-{index}", span { "UID" } input { id: "asset-uid-{index}", r#type: "number", min: "0", required: true, value: "{value.read()[index].uid}", oninput: move |e| value.write()[index].uid = e.value() } }
                            label { class: "field", r#for: "asset-gid-{index}", span { "GID" } input { id: "asset-gid-{index}", r#type: "number", min: "0", required: true, value: "{value.read()[index].gid}", oninput: move |e| value.write()[index].gid = e.value() } }
                            label { class: "field", r#for: "asset-mode-{index}", span { "Permissions (octal)" } input { id: "asset-mode-{index}", required: true, value: "{value.read()[index].mode}", oninput: move |e| value.write()[index].mode = e.value() } }
                        }
                    }
                }
            }
            if value.read().is_empty() { p { class: "small muted", "No assets attached." } }
            button { r#type: "button", onclick: move |_| value.write().push(AttachmentForm::default()), "+ Attach asset" }
            p { class: "small muted", "Upload reusable files from Assets in the sidebar. Files are copied to their destinations when the VM boots. Custom images must support boot file installation." }
        }
    }
}
#[cfg(test)]
mod tests;
