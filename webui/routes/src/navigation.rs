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

fn current_vm_id() -> String {
    web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .and_then(|hash| hash.strip_prefix("#vms/").map(str::to_owned))
        .unwrap_or_default()
}

pub fn use_vm_id() -> Signal<String> {
    let mut id = use_signal(current_vm_id);
    let listener = use_hook(move || {
        let listener = Rc::new(Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            id.set(current_vm_id());
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
