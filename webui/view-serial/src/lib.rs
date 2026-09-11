mod logs;
mod terminal;

use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub fn Serial(vm: Value) -> Element {
    let id = vm["id"].as_str().unwrap_or_default().to_owned();
    let enabled = vm["spec"]["terminal"].as_bool().unwrap_or(false);
    let mut terminal = use_signal(|| false);
    let tty = terminal() && enabled;
    let tabs = rsx! {
        div { class: "tabs serial-tabs",
            button { class: if !tty { "active" }, onclick: move |_| terminal.set(false), "Serial output" }
            button { class: if tty { "active" }, disabled: !enabled,
                title: if enabled { "Guest ttyS0 stream" } else { "Enable guest serial input in VM configuration." },
                onclick: move |_| terminal.set(true), "TTY Stream"
            }
        }
    };
    rsx! {
        style { {include_str!("serial.css")} }
        if tty {
            for id in [id] {
                terminal::TerminalPane { key: "terminal-{id}", id, tabs: tabs.clone() }
            }
        } else {
            for id in [id] {
                logs::LogOutput { key: "serial-{id}", id, stream: "serial", tabs: Some(tabs.clone()) }
            }
        }
    }
}

#[component]
pub fn FirecrackerLogs(id: String) -> Element {
    rsx! {
        style { {include_str!("serial.css")} }
        for id in [id] { logs::LogOutput { key: "firecracker-{id}", id, stream: "firecracker" } }
    }
}
