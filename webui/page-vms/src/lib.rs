//! VM inventory, live resource totals, creation, and selected VM details.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{get, text};
use firemage_webui_provider_auth::use_auth;
use firemage_webui_view_vm_detail::VmDetail;
use firemage_webui_view_vm_editor::VmEditor;
#[component]
pub fn Vms() -> Element {
    let auth = use_auth();
    let mut refresh = use_signal(|| 0u32);
    let mut creating = use_signal(|| false);
    let mut selected = use_signal(String::new);
    let mut filter = use_signal(String::new);
    let mut state_filter = use_signal(String::new);
    let rows = use_resource(move || {
        let _ = refresh();
        async move { get("/v1/vms").await }
    });
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            refresh += 1;
        }
    });
    let list = rows
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let running = list.iter().filter(|v| v["state"] == "running").count();
    let cpus: u64 = list
        .iter()
        .map(|v| v["spec"]["vcpus"].as_u64().unwrap_or(0))
        .sum();
    let memory: u64 = list
        .iter()
        .map(|v| v["spec"]["memory_mib"].as_u64().unwrap_or(0))
        .sum();
    let chosen = list
        .iter()
        .find(|v| v["id"].as_str() == Some(&selected()))
        .cloned();
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "COMPUTE" }
                h1 { "Virtual machines" }
                p { class: "muted", "Define, inspect, and control your microVMs." }
            }
            if auth.is_admin() {
                button { class: "primary", onclick: move |_| creating.set(true), "+ Create VM" }
            }
        }
        div { class: "metrics",
            Metric {
                label: "Running",
                value: running.to_string(),
                unit: format!("/ {}", list.len()),
            }
            Metric { label: "Configured vCPUs", value: cpus.to_string() }
            Metric {
                label: "Configured memory",
                value: format!("{:.1}", memory as f64 / 1024.),
                unit: "GiB",
            }
            div { class: "metric live-metric",
                span { "Inventory" }
                strong {
                    span { class: "live-dot" }
                    "Live"
                }
                small { "Refreshes every 5 seconds" }
            }
        }
        if let Some(Err(error)) = rows.read().as_ref() {
            Notice { message: error.clone() }
        }
        div { class: if chosen.is_some() { "workspace with-detail" } else { "workspace" },
            section { class: "inventory",
                div { class: "toolbar",
                    input {
                        id: "vm-search",
                        r#type: "search",
                        "aria-label": "Filter virtual machines",
                        placeholder: "Filter by name…",
                        value: "{filter}",
                        oninput: move |e| filter.set(e.value()),
                    }
                    select {
                        "aria-label": "Filter by state",
                        value: "{state_filter}",
                        onchange: move | e |
                                                        state_filter.set(e.value()),
                        option { value: "", "All states" }
                        for state in ["defined", "ready", "running", "paused", "stopped", "failed"] {
                            option { value: state, "{state}" }
                        }
                    }
                    button {
                        "aria-label": "Refresh virtual machines",
                        onclick: move |_| refresh += 1,
                        Icon { name: "refresh" }
                    }
                }
                if list.is_empty() {
                    Empty {
                        title: "No virtual machines yet",
                        description: if auth.is_admin() { "Create a VM from an OCI image, a local disk, or verified remote assets." } else { "Your virtual machines will appear here." },
                    }
                } else {
                    table {
                        thead {
                            tr {
                                th { "NAME / SOURCE" }
                                th { "STATE" }
                                th { "CPU / MEMORY" }
                            }
                        }
                        tbody {
                            for vm in list.iter()
                                .filter(|v| {
                                    text(&v["spec"], "name").to_lowercase().contains(&filter().to_lowercase())
                                        && (state_filter().is_empty() || v["state"] == state_filter())
                                })
                            {
                                tr {
                                    key: r#"{vm["id"].as_str().unwrap_or_default()}"#,
                                    class: if vm["id"] == selected() { "selected" } else { "" },
                                    td {
                                        button {
                                            class: "table-link",
                                            onclick: {
                                                let id = text(vm, "id");
                                                move |_| selected.set(id.clone())
                                            },
                                            r#"{vm["spec"]["name"].as_str().unwrap_or_default()}"#
                                        }
                                        small {
                                            r#"{vm["spec"]["rootfs"]["kind"].as_str().unwrap_or("External socket")}"#
                                        }
                                    }
                                    td {
                                        Status { value: text(vm, "state") }
                                    }
                                    td { class: "mono",
                                        r#"{vm["spec"]["vcpus"]} / {vm["spec"]["memory_mib"]} MiB"#
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(vm) = chosen.clone() {
                VmDetail {
                    key: r#"{vm["id"].as_str().unwrap_or_default()}"#,
                    vm,
                    onchanged: move | _ | refresh
                                                += 1,
                    onclose: move |_| selected.set(String::new()),
                }
            }
        }
        if creating() {
            VmEditor {
                onclose: move |_| creating.set(false),
                onsaved: move |_| {
                    creating.set(false);
                    refresh += 1;
                },
            }
        }
    }
}
