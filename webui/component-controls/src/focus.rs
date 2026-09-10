use wasm_bindgen::JsCast;

pub fn active() -> Option<web_sys::HtmlElement> {
    web_sys::window()?
        .document()?
        .active_element()?
        .dyn_into()
        .ok()
}

/// Keep keyboard navigation inside the most recently opened dialog.
pub fn trap_tab(backwards: bool) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Ok(dialogs) = document.query_selector_all("[role=dialog]") else {
        return;
    };
    let Some(dialog) = dialogs
        .item(dialogs.length().saturating_sub(1))
        .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let Ok(nodes) = dialog.query_selector_all("button:not([disabled]),a[href],summary,input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[tabindex='0']") else { return; };
    let elements: Vec<web_sys::HtmlElement> = (0..nodes.length())
        .filter_map(|index| nodes.item(index)?.dyn_into().ok())
        .filter(|element: &web_sys::HtmlElement| {
            element.offset_width() > 0 || element.offset_height() > 0
        })
        .collect();
    if elements.is_empty() {
        return;
    }
    let focused = active();
    let index = elements
        .iter()
        .position(|element| Some(element) == focused.as_ref());
    let next = match (index, backwards) {
        (Some(0) | None, true) => elements.len() - 1,
        (Some(index), true) => index - 1,
        (Some(index), false) => (index + 1) % elements.len(),
        (None, false) => 0,
    };
    let _ = elements[next].focus();
}
