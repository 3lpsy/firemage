use crate::Page;
use dioxus::prelude::*;
use std::rc::Rc;
use wasm_bindgen::{JsCast, closure::Closure};

fn current_page() -> Page {
    web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .map(|hash| Page::from_slug(hash.trim_start_matches('#')))
        .unwrap_or(Page::Vms)
}

/// URL fragments keep sidebar navigation addressable without server routes.
pub fn use_page() -> Signal<Page> {
    let mut page = use_signal(current_page);
    let listener = use_hook(move || {
        let listener = Rc::new(Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            let next = current_page();
            if page() != next {
                page.set(next);
            }
        }));
        if let Some(window) = web_sys::window() {
            let _ = window.add_event_listener_with_callback(
                "hashchange",
                listener.as_ref().as_ref().unchecked_ref(),
            );
        }
        listener
    });
    use_drop(move || {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                "hashchange",
                listener.as_ref().as_ref().unchecked_ref(),
            );
        }
    });
    use_effect(move || {
        let selected = page();
        if current_page() != selected {
            navigate(selected);
        }
    });
    page
}

pub fn navigate(page: Page) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(page.slug());
    }
}

fn current_path(prefix: &str) -> String {
    web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .and_then(|hash| hash.strip_prefix(prefix).map(str::to_owned))
        .unwrap_or_default()
}

pub fn use_vm_id() -> Signal<String> {
    use_path("#vms/")
}

pub fn use_egress_path() -> Signal<String> {
    use_path("#egress/")
}

fn use_path(prefix: &'static str) -> Signal<String> {
    let mut id = use_signal(|| current_path(prefix));
    let listener = use_hook(move || {
        let listener = Rc::new(Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            id.set(current_path(prefix));
        }));
        if let Some(window) = web_sys::window() {
            let _ = window.add_event_listener_with_callback(
                "hashchange",
                listener.as_ref().as_ref().unchecked_ref(),
            );
        }
        listener
    });
    use_drop(move || {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback(
                "hashchange",
                listener.as_ref().as_ref().unchecked_ref(),
            );
        }
    });
    id
}

pub fn navigate_vm(id: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_hash(&format!("vms/{id}"));
    }
}

pub fn navigate_vm_editor(id: Option<&str>) {
    if let Some(window) = web_sys::window() {
        let path = id.map_or_else(|| "vms/new".into(), |id| format!("vms/{id}/edit"));
        let _ = window.location().set_hash(&path);
    }
}
