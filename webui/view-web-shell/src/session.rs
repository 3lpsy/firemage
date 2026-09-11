use dioxus::prelude::*;
use firemage_webui_component_controls::Info;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/shell.js")]
extern "C" {
    #[wasm_bindgen(js_name = mountShell)]
    fn mount_shell(container: &str, status: &str, action: &str, id: &str, csrf: &str) -> JsValue;
    #[wasm_bindgen(js_name = disposeShell)]
    fn dispose_shell(handle: &JsValue);
}

#[component]
pub fn Session(id: String, csrf: String) -> Element {
    let handle = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<JsValue>)));
    let cleanup = handle.clone();
    use_drop(move || {
        if let Some(value) = cleanup.borrow_mut().take() {
            dispose_shell(&value);
        }
    });
    let container = format!("shell-{id}");
    let status = format!("shell-status-{id}");
    let action = format!("shell-action-{id}");
    let mount_container = container.clone();
    let mount_status = status.clone();
    let mount_action = action.clone();
    rsx! {
        div { class: "shell-toolbar",
            h3 { "Web Shell" }
            div { class: "shell-actions",
                span { id: status, class: "shell-status", role: "status", "Connecting…" }
                button { id: action, r#type: "button", "Disconnect" }
                Info { title: "Web Shell", "Opens a separate guest shell using the configured command. Leaving this tab closes the shell and its processes. Reconnecting starts a new shell; displayed output is kept." }
            }
        }
        div { id: container, class: "web-shell-terminal", onmounted: move |_| {
            *handle.borrow_mut() = Some(mount_shell(&mount_container, &mount_status, &mount_action, &id, &csrf));
        } }
    }
}
