use crate::files::{FileFields, prepare};
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};

#[component]
pub fn BootEditor(
    vm: Value,
    onclose: EventHandler<()>,
    onsaved: EventHandler<()>,
    #[props(default)] onapply: Option<EventHandler<Value>>,
) -> Element {
    let auth = use_auth();
    let mut files = use_signal(|| {
        vm["spec"]["files"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|file| {
                let mut file = file.clone();
                file["_mode"] = json!(format!("{:04o}", file["mode"].as_u64().unwrap_or(420)));
                file
            })
            .collect::<Vec<_>>()
    });
    let attachments = use_signal(|| firemage_webui_view_asset_attachments::from_spec(&vm["spec"]));
    let userdata = use_signal(|| text(&vm["spec"], "userdata"));
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: "Configure boot inputs", onclose,
            form { class: "modal-form", onsubmit: move |event| {
                event.prevent_default();
                if busy() { return; }
                let prepared = match files.read().iter().enumerate().map(|(index,file)| prepare(file,index)).collect::<Result<Vec<_>,_>>() {
                    Ok(files) => files, Err(message) => { error.set(message); return; }
                };
                let attached = match firemage_webui_view_asset_attachments::attachments(&attachments.read()) {
                    Ok(value) => value, Err(message) => { error.set(message); return; }
                };
                let secrets = match firemage_webui_view_asset_attachments::secret_attachments(&attachments.read()) {
                    Ok(value) => value, Err(message) => { error.set(message); return; }
                };
                let mut spec = vm["spec"].clone(); spec["files"] = json!(prepared);
                spec["attachments"] = json!(attached);
                spec["secret_attachments"] = json!(secrets);
                if userdata().is_empty() { spec.as_object_mut().unwrap().remove("userdata"); } else { spec["userdata"] = json!(userdata()); }
                if let Some(onapply) = onapply { onapply.call(spec); return; }
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
                    firemage_webui_view_asset_attachments::Attachments { value: attachments }
                    p { class: "small muted", "Copy files into the guest. No host directories are shared." }
                    for index in 0..files.read().len() { FileFields { files, index, error } }
                    button { r#type: "button", onclick: move |_| files.write().push(json!({"path":"","destination":"","content":"","encoding":"utf8","uid":0,"gid":0,"_mode":"0644"})), "+ Add boot file" }
                    h3 { class: "egress-section", "Userdata"
                        Info { title: "Userdata execution order", "Userdata is a shell script run after networking, environment variables, and file copies are ready. The OCI guest runs it using /bin/sh. Custom images must implement the seed initialization sequence. Use secret environment references instead of putting credentials in this script." }
                    }
                    ShellEditor { label: "Post-setup script", id: "boot-userdata", value: userdata, rows: 8 }
                }
                if onapply.is_some() { p { class: "small muted", "Applied changes are saved when you submit the VM form." } }
                div { class: "actions end",
                    button { r#type: "button", onclick: move |_| onclose.call(()), "Cancel" }
                    button { r#type: "submit", class: "primary", disabled: busy(), if onapply.is_some() { "Apply to draft" } else { "Save boot inputs" } }
                }
            }
        }
    }
}
