use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{pretty, request};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn Metadata(id: String, initial: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let value = use_signal(|| pretty(&initial));
    let mut message = use_signal(String::new);
    let mut success = use_signal(|| false);
    rsx! {
        h3 {
            "Instance metadata"
            Info { title: "Metadata service",
                "Firecracker MMDS v2 serves JSON inside a guest with a network interface. Use an isolated network when the guest needs metadata but no external access. The guest must request an MMDS session token."
            }
        }
        Editor {
            label: "Metadata JSON",
            id: "vm-metadata",
            value,
            rows: 12,
            disabled: !auth.is_admin(),
        }
        Notice { message: message(), success: success() }
        if auth.is_admin() {
            button {
                class: "primary",
                onclick: move |_| {
                    let id = id.clone();
                    let body = match serde_json::from_str::<Value>(&value()) {
                        Ok(value) => value,
                        Err(e) => {
                            success.set(false);
                            message.set(e.to_string());
                            return;
                        }
                    };
                    spawn(async move {
                        match request(
                                "POST",
                                &format!("/v1/vms/{id}/actions"),
                                Some(json!({ "action" : "metadata", "value" : body })),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => {
                                success.set(true);
                                message.set("Metadata updated".into());
                                onchanged.call(());
                            }
                            Err(e) => {
                                success.set(false);
                                message.set(e);
                            }
                        }
                    });
                },
                "Update metadata"
            }
        }
    }
}
