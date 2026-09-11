use crate::Icon;
use dioxus::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/clipboard.js")]
extern "C" {
    #[wasm_bindgen(js_name = copyContent)]
    fn copy_content(kind: &str, value: &str) -> js_sys::Promise;
}

#[derive(Clone, PartialEq)]
pub enum CopySource {
    Text(String),
    File(String),
    Terminal(String),
}

#[component]
pub fn CopyButton(source: CopySource, label: String, #[props(default)] disabled: bool) -> Element {
    let mut busy = use_signal(|| false);
    let mut feedback = use_signal(String::new);
    rsx! {
        button {
            class: "icon-button", r#type: "button", title: label.clone(),
            "aria-label": label, disabled: disabled || busy(),
            onclick: move |_| {
                let (kind, value) = match source.clone() {
                    CopySource::Text(value) => ("text", value),
                    CopySource::File(value) => ("file", value),
                    CopySource::Terminal(value) => ("terminal", value),
                };
                busy.set(true);
                feedback.set(String::new());
                let result = copy_content(kind, &value);
                spawn(async move {
                    let error = wasm_bindgen_futures::JsFuture::from(result).await
                        .map(|value| value.as_string().unwrap_or_default())
                        .unwrap_or_else(|_| "Copy failed".into());
                    feedback.set(if error.is_empty() { "Copied".into() } else { error });
                    busy.set(false);
                });
            },
            Icon { name: "copy" }
        }
        if !feedback().is_empty() { span { class: "small clipboard-feedback", role: "status", "{feedback}" } }
    }
}
