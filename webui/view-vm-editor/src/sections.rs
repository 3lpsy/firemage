use dioxus::prelude::*;

pub const SECTIONS: &[(&str, &str)] = &[
    ("machine", "Machine"),
    ("storage", "Image and disks"),
    ("workload", "Workload"),
    ("network", "Network"),
    ("egress", "Egress"),
    ("environment", "Environment"),
    ("attachments", "Attachments"),
    ("boot", "Boot inputs"),
    ("security", "Security limits"),
    ("metadata", "Metadata"),
    ("terminal", "Terminal"),
];

#[component]
pub fn SectionIndex() -> Element {
    rsx! {
        nav { class: "vm-form-index", "aria-label": "VM configuration sections",
            for &(id, title) in SECTIONS {
                button { r#type: "button", "aria-controls": "vm-section-{id}", onclick: move |_| open(id), "{title}" }
            }
            p { class: "small muted", "All changes save together." }
        }
    }
}

fn open(id: &str) {
    let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&format!("vm-section-{id}")))
    else {
        return;
    };
    let _ = element.set_attribute("open", "");
    element.scroll_into_view();
}

#[component]
pub fn Section(
    id: String,
    title: String,
    summary: String,
    #[props(default)] expanded: bool,
    children: Element,
) -> Element {
    rsx! {
        details { id: "vm-section-{id}", class: "vm-form-section", open: expanded,
            summary { strong { "{title}" } span { "{summary}" } }
            div { class: "vm-form-section-body", {children} }
        }
    }
}
