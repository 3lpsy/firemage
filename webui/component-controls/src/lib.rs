//! Form controls, dialogs, notices, and compact resource summaries.
use dioxus::prelude::*;
mod clipboard;
mod focus;
mod icon;
mod secret_field_label;
pub use clipboard::{CopyButton, CopySource};
pub use icon::Icon;
pub use secret_field_label::SecretFieldLabel;
#[component]
pub fn Field(
    label: String,
    id: String,
    value: Signal<String>,
    #[props(default = "text".into())] kind: String,
    #[props(default)] placeholder: String,
    #[props(default)] required: bool,
    #[props(default)] disabled: bool,
) -> Element {
    rsx! {
        label { class: "field", r#for: "{id}",
            span { "{label}" }
            input {
                id: "{id}",
                r#type: "{kind}",
                value: "{value}",
                placeholder: "{placeholder}",
                required,
                disabled,
                oninput: move |e| value.set(e.value()),
            }
        }
    }
}
#[component]
pub fn Editor(
    label: String,
    id: String,
    value: Signal<String>,
    #[props(default = 12)] rows: u32,
    #[props(default)] disabled: bool,
) -> Element {
    rsx! {
        label { class: "field", r#for: "{id}",
            span { "{label}" }
            textarea {
                class: "code-editor",
                id: "{id}",
                rows,
                spellcheck: "false",
                value: "{value}",
                disabled,
                oninput: move |e| value.set(e.value()),
            }
        }
    }
}
#[component]
pub fn Notice(message: String, #[props(default)] success: bool) -> Element {
    if message.is_empty() {
        return rsx! {};
    }
    rsx! {
        div {
            key: "{message}",
            class: if success { "notice success" } else { "notice error" },
            role: if success { "status" } else { "alert" },
            onmounted: move |event| async move {
                if !success { let _ = event.scroll_to(dioxus::html::ScrollBehavior::Instant).await; }
            },
            "{message}"
        }
    }
}
#[component]
pub fn Info(title: String, children: Element) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        button {
            class: "info-button",
            r#type: "button",
            title: "{title}",
            "aria-label": "About {title}",
            onclick: move |_| open.set(true),
            "i"
        }
        if open() {
            Modal { title: title.clone(), onclose: move |_| open.set(false),
                div { class: "explanation", {children} }
            }
        }
    }
}
#[component]
pub fn Modal(title: String, onclose: EventHandler<()>, children: Element) -> Element {
    let previous = use_hook(focus::active);
    use_drop(move || {
        if let Some(element) = previous {
            let _ = element.focus();
        }
    });
    rsx! {
        div { class: "modal-backdrop", onclick: move |_| onclose.call(()),
            div {
                class: "modal",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "{title}",
                tabindex: "-1",
                onmounted: move |event| async move {
                    let _ = event.set_focus(true).await;
                },
                onclick: move |e| e.stop_propagation(),
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        e.stop_propagation();
                        onclose.call(());
                    } else if e.key() == Key::Tab {
                        e.prevent_default();
                        e.stop_propagation();
                        focus::trap_tab(e.modifiers().shift());
                    }
                },
                header {
                    h2 { "{title}" }
                    button {
                        class: "icon-button",
                        r#type: "button",
                        "aria-label": "Close dialog",
                        onclick: move |_| onclose.call(()),
                        Icon { name: "close" }
                    }
                }
                div { class: "modal-content", {children} }
            }
        }
    }
}
#[component]
pub fn Confirm(
    title: String,
    description: String,
    #[props(default = "Confirm".into())] label: String,
    onconfirm: EventHandler<()>,
    onclose: EventHandler<()>,
) -> Element {
    rsx! {
        Modal { title, onclose,
            p { class: "muted", "{description}" }
            div { class: "actions end",
                button { onclick: move |_| onclose.call(()), "Cancel" }
                button { class: "danger", onclick: move |_| onconfirm.call(()), "{label}" }
            }
        }
    }
}
#[component]
pub fn Status(value: String) -> Element {
    rsx! {
        span { class: "status status-{value}",
            span { class: "status-dot" }
            "{value}"
        }
    }
}
#[component]
pub fn Empty(title: String, description: String) -> Element {
    rsx! {
        div { class: "empty",
            div { class: "empty-mark",
                Icon { name: "vms", size: 32 }
            }
            h3 { "{title}" }
            p { "{description}" }
        }
    }
}
#[component]
pub fn Metric(label: String, value: String, #[props(default)] unit: String) -> Element {
    rsx! {
        div { class: "metric",
            span { "{label}" }
            strong {
                "{value}"
                small { "{unit}" }
            }
        }
    }
}
