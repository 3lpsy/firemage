use dioxus::prelude::*;
use firemage_webui_component_controls::{Field, Info};

#[component]
pub fn Settings(
    enabled: Signal<bool>,
    command: Signal<String>,
    #[props(default)] disabled: bool,
) -> Element {
    rsx! {
        div { class: "switch-row",
            span { class: "actions",
                label { r#for: "vm-web-terminal", "Enable Web Terminal" }
                Info { title: "Web Terminal",
                    "Adds firemage-guest to the input disk and starts it during managed OCI initialization. A private vsock connection provides a separate interactive shell. Custom images must support the guest helper and start it from their own init, with a compatible kernel and shell. The helper is added to disk only when enabled."
                }
            }
            input { id: "vm-web-terminal", class: "toggle-switch", r#type: "checkbox", role: "switch", checked: enabled(), disabled,
                onchange: move |event| enabled.set(event.checked()) }
        }
        p { class: "small muted", "Adds a guest helper for an interactive shell in the browser." }
        if enabled() {
            Field { label: "Shell command (JSON arguments)", id: "vm-shell-command", value: command, disabled, placeholder: r#"["/bin/sh", "-i"]"# }
        }
    }
}
