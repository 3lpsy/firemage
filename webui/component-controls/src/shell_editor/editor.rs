use super::tokens::highlight;
use dioxus::prelude::*;

/// Shell highlighting over a native textarea; the painted layer is never editable or announced.
#[component]
pub fn ShellEditor(
    label: String,
    id: String,
    mut value: Signal<String>,
    #[props(default = 12)] rows: u32,
    #[props(default)] disabled: bool,
) -> Element {
    let mut scroll = use_signal(|| (0, 0));
    let source = value();
    let tokens = highlight(&source);
    let scroll_id = id.clone();
    rsx! {
        style { {include_str!("style.css")} }
        label { class: "field", r#for: "{id}",
            span { "{label}" }
            div { class: "shell-editor",
                pre { class: "shell-editor-paint", "aria-hidden": "true",
                    code { style: "transform:translate({-scroll().0}px, {-scroll().1}px)",
                        for token in tokens {
                            span { class: token.class, "{token.text}" }
                        }
                        "\n"
                    }
                }
                textarea {
                    class: "code-editor shell-editor-input", id: "{id}", rows, disabled,
                    spellcheck: "false", autocapitalize: "off", autocomplete: "off", wrap: "off",
                    value: source.clone(),
                    oninput: move |event| value.set(event.value()),
                    onscroll: move |_| {
                        if let Some(element) = web_sys::window().and_then(|window| window.document()).and_then(|document| document.get_element_by_id(&scroll_id)) {
                            scroll.set((element.scroll_left(), element.scroll_top()));
                        }
                    },
                }
            }
        }
    }
}
