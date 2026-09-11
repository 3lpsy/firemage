use dioxus::prelude::*;
use firemage_webui_provider_auth::use_auth;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/terminal.js")]
extern "C" {
    #[wasm_bindgen(js_name = mountTerminal)]
    fn mount_terminal(container: &str, status: &str, id: &str, csrf: &str) -> JsValue;
    #[wasm_bindgen(js_name = disposeTerminal)]
    fn dispose_terminal(handle: &JsValue);
}

#[component]
pub fn TerminalPane(id: String) -> Element {
    let auth = use_auth();
    let mut connected = use_signal(|| false);
    rsx! {
        p { class: "muted small", "ttyS0 input reaches the guest console program. The guest must provide its own shell or getty. Serial output is shared with the workload." }
        if auth.is_admin() {
            button { onclick: move |_| connected.toggle(),
                if connected() { "Disconnect" } else { "Connect terminal" }
            }
            if connected() { TerminalSession { id, csrf: auth.csrf() } }
        } else {
            p { class: "muted", "Administrator access is required for terminal input." }
        }
    }
}

#[component]
fn TerminalSession(id: String, csrf: String) -> Element {
    let handle = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<JsValue>)));
    let cleanup = handle.clone();
    use_drop(move || {
        if let Some(value) = cleanup.borrow_mut().take() {
            dispose_terminal(&value);
        }
    });
    let container = format!("terminal-{id}");
    let status = format!("terminal-status-{id}");
    let mount_container = container.clone();
    let mount_status = status.clone();
    rsx! {
        p { id: status, role: "status", "Connecting…" }
        div { id: container, class: "guest-terminal", onmounted: move |_| {
            *handle.borrow_mut() = Some(mount_terminal(&mount_container, &mount_status, &id, &csrf));
        } }
    }
}
