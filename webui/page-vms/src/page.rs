use dioxus::prelude::*;
use firemage_webui_component_controls::Notice;
use firemage_webui_provider_api::{encode, get};
use firemage_webui_view_vm_detail::VmDetail;

#[component]
pub fn VmPage(id: String, tab: firemage_webui_routes::VmTab) -> Element {
    let mut refresh = use_signal(|| 0u32);
    let vm = use_resource(move || {
        let _ = refresh();
        let id = encode(&id);
        async move { get(&format!("/v1/vms/{id}")).await }
    });
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            refresh += 1;
        }
    });
    rsx! {
        div { class: "vm-page-breadcrumb", a { href: "#vms", "Virtual machines" } span { "/" } span { "VM details" } }
        match vm.read().as_ref() {
            Some(Ok(vm)) => rsx! { VmDetail {
                vm: vm.clone(), full_page: true, selected_tab: tab,
                onchanged: move |_| refresh += 1,
                onclose: move |_| firemage_webui_routes::navigate(firemage_webui_routes::Page::Vms),
            } },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { "Loading VM…" } },
        }
    }
}

#[component]
pub fn CreateVmPage() -> Element {
    let auth = firemage_webui_provider_auth::use_auth();
    if !auth.is_admin() {
        return rsx! { Notice { message: "Administrator access is required to create a VM." } };
    }
    rsx! { firemage_webui_view_vm_editor::VmEditor {
        onclose: move |_| firemage_webui_routes::navigate(firemage_webui_routes::Page::Vms),
        onsaved: move |id: String| firemage_webui_routes::navigate_vm(&id),
    } }
}

#[component]
pub fn EditVmPage(id: String) -> Element {
    let auth = firemage_webui_provider_auth::use_auth();
    let vm_id = id.clone();
    let vm = use_resource(move || {
        let id = encode(&id);
        async move { get(&format!("/v1/vms/{id}")).await }
    });
    if !auth.is_admin() {
        return rsx! { Notice { message: "Administrator access is required to edit a VM." } };
    }
    rsx! {
        match vm.read().as_ref() {
            Some(Ok(vm)) if matches!(vm["state"].as_str(), Some("defined" | "stopped" | "failed")) => rsx! { firemage_webui_view_vm_editor::VmEditor {
                key: "{vm_id}", vm: vm.clone(),
                onclose: move |_| firemage_webui_routes::navigate_vm(&vm_id),
                onsaved: move |id: String| firemage_webui_routes::navigate_vm(&id),
            } },
            Some(Ok(_)) => rsx! {
                Notice { message: "Stop the VM before editing its configuration." }
                a { href: "#vms/{vm_id}/overview", "Back to virtual machine" }
            },
            Some(Err(error)) => rsx! { Notice { message: error.clone() } },
            None => rsx! { p { "Loading VM…" } },
        }
    }
}
