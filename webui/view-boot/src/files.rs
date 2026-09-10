use base64::Engine;
use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn FileFields(
    mut files: Signal<Vec<Value>>,
    index: usize,
    mut error: Signal<String>,
) -> Element {
    let file = files.read()[index].clone();
    rsx! {
        fieldset { class: "egress-rule",
            legend { "Boot file {index + 1}" }
            div { class: "heading compact",
                label { class: "field", r#for: "boot-{index}-upload", span { "Upload a file" }
                    input { id: "boot-{index}-upload", r#type: "file", onchange: move |event| {
                        let Some(upload) = event.files().into_iter().next() else { return; };
                        if upload.size() > 4 * 1024 * 1024 { error.set("Boot uploads are limited to 4 MiB each in this editor.".into()); return; }
                        spawn(async move {
                            match upload.read_bytes().await {
                                Ok(bytes) => {
                                    if let Some(file) = files.write().get_mut(index) {
                                        file["content"] = json!(base64::engine::general_purpose::STANDARD.encode(bytes));
                                        file["encoding"] = json!("base64");
                                        if file["path"].as_str().is_none_or(str::is_empty) { file["path"] = json!(upload.name()); }
                                    }
                                },
                                Err(message) => error.set(format!("Could not read file: {message}")),
                            }
                        });
                    } }
                }
                button { r#type: "button", class: "danger subtle", "aria-label": "Remove boot file {index + 1}", onclick: move |_| { files.write().remove(index); }, "Remove" }
            }
            FileField { files, index, name: "path", label: "Seed file name" }
            FileField { files, index, name: "destination", label: "Absolute guest destination (optional)" }
            p { class: "small muted", "Leave destination empty to keep the file on the seed disk." }
            div { class: "form-grid",
                FileField { files, index, name: "uid", label: "Owner UID", kind: "number" }
                FileField { files, index, name: "gid", label: "Group GID", kind: "number" }
                FileField { files, index, name: "_mode", label: "Permissions (octal)" }
            }
            div { class: "radio-group",
                for (encoding,label) in [("utf8","Text"),("base64","Base64")] {
                    label { input { r#type: "radio", name: "boot-{index}-encoding", checked: file["encoding"] == encoding,
                        onchange: move |_| files.write()[index]["encoding"] = json!(encoding),
                    } "{label}" }
                }
            }
            label { class: "field", r#for: "boot-{index}-content", span { "File contents" }
                textarea { class: "code-editor", id: "boot-{index}-content", rows: 5, value: file["content"].as_str().unwrap_or_default(),
                    oninput: move |event| files.write()[index]["content"] = json!(event.value()),
                }
            }
        }
    }
}
#[component]
fn FileField(
    mut files: Signal<Vec<Value>>,
    index: usize,
    name: &'static str,
    label: &'static str,
    #[props(default = "text")] kind: &'static str,
) -> Element {
    let value = match &files.read()[index][name] {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => String::new(),
    };
    rsx! { label { class: "field", r#for: "boot-{index}-{name}", span { "{label}" }
        input { id: "boot-{index}-{name}", r#type: kind, value, oninput: move |event| files.write()[index][name] = json!(event.value()) }
    } }
}
pub fn prepare(value: &Value, index: usize) -> Result<Value, String> {
    let mut file = value.clone();
    if file["path"].as_str().is_none_or(str::is_empty) {
        return Err(format!("Boot file {} needs a seed file name.", index + 1));
    }
    for name in ["uid", "gid"] {
        let value = file[name]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| file[name].as_u64().unwrap_or(0).to_string());
        file[name] = json!(
            value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be a numeric guest ID."))?
        );
    }
    let mode = u32::from_str_radix(file["_mode"].as_str().unwrap_or("0644"), 8)
        .map_err(|_| "Use octal permissions such as 0644 or 0600.")?;
    if mode > 0o777 {
        return Err("File permissions must be between 0000 and 0777.".into());
    }
    file["mode"] = json!(mode);
    file.as_object_mut().unwrap().remove("_mode");
    if file["destination"].as_str().is_none_or(str::is_empty) {
        file.as_object_mut().unwrap().remove("destination");
    } else if !file["destination"].as_str().unwrap().starts_with('/') {
        return Err("Guest destinations must be absolute paths.".into());
    }
    Ok(file)
}
