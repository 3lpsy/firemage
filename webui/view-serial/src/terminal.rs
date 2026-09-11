use dioxus::prelude::*;
use firemage_webui_component_controls::{CopyButton, CopySource, Info};
use firemage_webui_provider_auth::use_auth;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/terminal.js")]
extern "C" {
    #[wasm_bindgen(js_name = mountTerminal)]
    fn mount_terminal(container: &str, status: &str, action: &str, id: &str, csrf: &str)
    -> JsValue;
    #[wasm_bindgen(js_name = disposeTerminal)]
    fn dispose_terminal(handle: &JsValue);
}

#[component]
pub fn TerminalPane(id: String, tabs: Element) -> Element {
    let auth = use_auth();
    rsx! {
        if auth.is_admin() {
            TerminalSession { id, tabs, csrf: auth.csrf() }
        } else {
            div { class: "serial-toolbar", {tabs} }
            p { class: "muted", "Administrator access is required for serial input." }
        }
    }
}

#[component]
fn TerminalSession(id: String, tabs: Element, csrf: String) -> Element {
    let handle = use_hook(|| std::rc::Rc::new(std::cell::RefCell::new(None::<JsValue>)));
    let cleanup = handle.clone();
    use_drop(move || {
        if let Some(value) = cleanup.borrow_mut().take() {
            dispose_terminal(&value);
        }
    });
    let container = format!("terminal-{id}");
    let status = format!("terminal-status-{id}");
    let action = format!("terminal-action-{id}");
    let mount_container = container.clone();
    let mount_status = status.clone();
    let mount_action = action.clone();
    rsx! {
        div { class: "serial-toolbar",
            {tabs}
            div { class: "serial-actions",
                span { id: status, class: "serial-connection", role: "status", "Connecting…" }
                button { id: action, r#type: "button", "Disconnect" }
                Info { title: "TTY Stream", "Displays guest ttyS0 output and sends keyboard input to its console program. The guest must already provide a shell or getty. Opening this tab connects automatically." }
            }
        }
        div { class: "actions end", CopyButton { source: CopySource::Terminal(container.clone()), label: "Copy TTY stream" } }
        div { id: container, class: "guest-terminal", onmounted: move |_| {
            *handle.borrow_mut() = Some(mount_terminal(&mount_container, &mount_status, &mount_action, &id, &csrf));
        } }
    }
}
