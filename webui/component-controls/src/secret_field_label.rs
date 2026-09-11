use crate::Icon;
use dioxus::prelude::*;

#[component]
pub fn SecretFieldLabel(
    id: String,
    label: String,
    onrefresh: EventHandler<()>,
    help: Option<Element>,
) -> Element {
    rsx! {
        div { class: "secret-field-label",
            label { r#for: id, "{label}" }
            if let Some(help) = help { {help} }
            a { class: "secret-field-action", href: "#secrets", target: "_blank", rel: "noopener",
                title: "Create or manage secrets (opens in a new tab)",
                "aria-label": "Manage secrets for {label} (opens in a new tab)",
                Icon { name: "plus", size: 15 }
            }
            button { class: "secret-field-action", r#type: "button", title: "Refresh secrets",
                "aria-label": "Refresh secrets for {label}",
                onclick: move |_| onrefresh.call(()),
                Icon { name: "refresh", size: 15 }
            }
        }
    }
}
