use dioxus::prelude::*;
use firemage_webui_component_controls::{Icon, Modal, Notice};
use firemage_webui_provider_api::text;
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;

#[component]
pub fn VmSnapshots(vm: Value, onchanged: EventHandler<()>) -> Element {
    let auth = use_auth();
    let mut saving = use_signal(|| false);
    let mut restoring = use_signal(|| false);
    let mut rows = use_resource(crate::model::snapshots);
    let can_restore = matches!(
        text(&vm, "state").as_str(),
        "stopped" | "defined" | "failed"
    ) && vm["spec"]["security"]["mode"] == "jailed";
    rsx! {
        style { {include_str!("style.css")} }
        section { class: "snapshot-vm-controls",
            div { class: "heading compact vm-tab-heading",
                h3 { "Snapshots" }
                div { class: "actions",
                    button { disabled: !auth.is_admin() || !crate::model::is_capture_target(&vm), onclick: move |_| saving.set(true), "Save snapshot" }
                    button { disabled: !auth.is_admin() || !can_restore, onclick: move |_| restoring.set(true), "Restore snapshot" }
                    button { class: "icon-button", title: "Refresh snapshots", "aria-label": "Refresh VM snapshots", onclick: move |_| rows.restart(), Icon { name: "refresh", size: 16 } }
                }
            }
            p { class: "muted small", "Pause the VM to save a snapshot. Stop it to restore one. Snapshots include guest memory and disks." }
            match rows.read().as_ref() {
                Some(Ok(snapshots)) => {
                    let snapshots = snapshots.iter().filter(|snapshot| crate::model::is_source_vm(&vm, snapshot)).cloned().collect::<Vec<_>>();
                    rsx! {
                        if snapshots.is_empty() { p { class: "muted small", "No snapshots saved from this VM." } }
                        else { crate::inventory::Inventory { snapshots, vm_scope: true } }
                    }
                },
                Some(Err(error)) => rsx! { Notice { message: error.clone() } },
                None => rsx! { p { role: "status", "Loading snapshots…" } },
            }
        }
        if saving() { crate::save::Save { vm: vm.clone(), onclose: move |_| saving.set(false), onsaved: move |_| { saving.set(false); rows.restart(); onchanged.call(()); } } }
        if restoring() { RestorePicker { vm: vm.clone(), onclose: move |_| restoring.set(false), onchanged } }
    }
}

#[component]
fn RestorePicker(vm: Value, onclose: EventHandler<()>, onchanged: EventHandler<()>) -> Element {
    let mut rows = use_resource(crate::model::snapshots);
    let mut alias = use_signal(String::new);
    let choices = rows
        .read()
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|rows| {
            rows.iter()
                .filter(|snapshot| crate::model::is_restore_target(&vm, snapshot))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selected = choices
        .iter()
        .find(|snapshot| crate::model::snapshot_choice(snapshot) == alias())
        .cloned();
    rsx! {
        Modal { title: "Restore snapshot", onclose,
            label { class: "field", span { "Snapshot" }
                input { list: "vm-snapshot-options", value: r#"{alias}"#, placeholder: "Search snapshot aliases", oninput: move |event| alias.set(event.value()) }
            }
            datalist { id: "vm-snapshot-options", for snapshot in &choices { option { value: crate::model::snapshot_choice(snapshot), r#"{text(snapshot, "source_vm_name")}"# } } }
            if let Some(Err(message)) = rows.read().as_ref() { Notice { message: message.clone() } }
            if choices.is_empty() && rows.read().is_some() { p { class: "muted small", "No snapshots match this VM's CPU and memory settings." } }
            for snapshot in selected { crate::restore::Restore { key: r#"{text(&snapshot, "id")}"#, snapshot, vm: vm.clone(), onchanged: move |_| { rows.restart(); onchanged.call(()); } } }
        }
    }
}
