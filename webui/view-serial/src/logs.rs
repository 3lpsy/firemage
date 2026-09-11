use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::get;

#[component]
pub fn LogOutput(id: String, stream: String) -> Element {
    let title = if stream == "firecracker" {
        "Firecracker logs, latest launch"
    } else {
        "Guest serial"
    };
    let mut refresh = use_signal(|| 0u32);
    let mut legacy = use_signal(|| false);
    let mut output = use_resource(move || {
        let _ = refresh();
        let selected = if legacy() {
            "legacy".to_owned()
        } else {
            stream.clone()
        };
        let id = id.clone();
        async move { get(&format!("/v1/vms/{id}/logs?stream={selected}")).await }
    });
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(2000).await;
            refresh += 1;
        }
    });
    rsx! {
        div { class: "heading compact",
            h3 { if legacy() { "Legacy combined output" } else { "{title}" } }
            button { onclick: move |_| output.restart(), "Refresh" }
        }
        match output.read().as_ref() {
            Some(Ok(value)) => rsx! {
                if value["legacy_available"] == true {
                    button { onclick: move |_| legacy.toggle(),
                        if legacy() { "Back to separate stream" } else { "View legacy combined output" }
                    }
                    if legacy() {
                        p { class: "muted small", "Earlier runs combined guest serial and Firecracker diagnostics. New capture separates them after the VM stops and starts." }
                    }
                }
                pre { class: "console", r#"{value["text"].as_str().filter(|text| !text.is_empty()).unwrap_or("No output yet.")}"# }
                if let Some(stderr) = value["stderr"].as_str().filter(|text| !text.is_empty()) {
                    h3 { "Process stderr" }
                    pre { class: "console", "{stderr}" }
                }
            },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { class: "muted", "Loading output…" } },
        }
    }
}
