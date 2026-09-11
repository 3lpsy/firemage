//! Desktop browser entry point and authenticated application shell.
use dioxus::prelude::*;
use firemage_webui_component_controls::{Icon, Notice};
use firemage_webui_provider_auth::use_auth_provider;
use firemage_webui_routes::{Page, use_page};
fn main() {
    #[cfg(target_arch = "wasm32")]
    dioxus::launch(App);
}
#[component]
fn App() -> Element {
    let auth = use_auth_provider();
    let mut page = use_page();
    let mut error = use_signal(String::new);
    use_effect(move || {
        if auth.is_logged_in() && !auth.is_admin() && page().is_admin() {
            page.set(Page::Vms);
        }
    });
    if !*auth.ready.read() {
        return rsx! {
            div { class: "loading-screen",
                span { class: "brand-mark",
                    Icon { name: "brand", size: 27 }
                }
                p { "Connecting to Firemage…" }
            }
        };
    }
    if !auth.error.read().is_empty() {
        return rsx! {
            div { class: "loading-screen",
                h1 { "Unable to connect" }
                Notice { message: auth.error.read().clone() }
                a { class: "button", href: "/", "Retry" }
            }
        };
    }
    if !auth.is_logged_in() {
        return rsx! {
            firemage_webui_page_login::Login {}
        };
    }
    rsx! {
        div { class: "app-shell",
            aside { class: "sidebar",
                a { class: "brand", href: "/",
                    span { class: "brand-mark",
                        Icon { name: "brand", size: 27 }
                    }
                    "firemage"
                }
                div { class: "workspace-label", "CONTROL PLANE" }
                nav { "aria-label": "Main navigation",
                    for item in [Page::Vms, Page::Snapshots, Page::Kernels, Page::Assets, Page::Networks, Page::Activity, Page::Tokens, Page::Secrets, Page::Users, Page::Config] {
                        if !item.is_admin() || auth.is_admin() {
                            button {
                                class: if page() == item { "nav-item active" } else { "nav-item" },
                                "aria-current": if page() == item { "page" } else { "false" },
                                onclick: move |_| { firemage_webui_routes::navigate(item); page.set(item); },
                                span { class: "nav-icon",
                                    Icon { name: item.slug() }
                                }
                                "{item.label()}"
                            }
                        }
                    }
                }
                div { class: "sidebar-bottom",
                    button {
                        class: if page() == Page::Host { "nav-item active" } else { "nav-item" },
                        onclick: move |_| page.set(Page::Host),
                        span { class: "nav-icon",
                            Icon { name: "host" }
                        }
                        "Host"
                    }
                    div { class: "account",
                        div { class: "avatar",
                            r#"{auth.session.read()["user"]["username"].as_str().unwrap_or("?").chars().next().unwrap_or('?').to_uppercase()}"#
                        }
                        div {
                            strong {
                                r#"{auth.session.read()["user"]["username"].as_str().unwrap_or_default()}"#
                            }
                            small {
                                if auth.is_admin() {
                                    "Administrator"
                                } else {
                                    "Member"
                                }
                            }
                        }
                        button {
                            class: "icon-button",
                            "aria-label": "Sign out",
                            title: "Sign out",
                            onclick: move |_| {
                                spawn(async move {
                                    if let Err(e) = auth.logout().await {
                                        error.set(e);
                                    }
                                });
                            },
                            Icon { name: "logout", size: 17 }
                        }
                    }
                }
            }
            div { class: "main-shell",
                header { class: "topbar",
                    div { class: "breadcrumb",
                        "Workspace"
                        span { "/" }
                        strong { "{page().label()}" }
                    }
                    div { class: "connection",
                        span { class: "live-dot" }
                        "Connected"
                    }
                }
                main {
                    Notice { message: error() }
                    match if page().is_admin() && !auth.is_admin() { Page::Vms } else { page() } {
                        Page::Vms => rsx! {
                            firemage_webui_page_vms::Vms {}
                        },
                        Page::Snapshots => rsx! { firemage_webui_page_snapshots::Snapshots {} },
                        Page::Networks => rsx! {
                            firemage_webui_page_networks::Networks {}
                        },
                        Page::Kernels => rsx! { firemage_webui_page_kernels::Kernels {} },
                        Page::Assets => rsx! { firemage_webui_page_assets::Assets {} },
                        Page::Activity => rsx! {
                            firemage_webui_page_activity::Activity {}
                        },
                        Page::Tokens => rsx! {
                            firemage_webui_page_tokens::Tokens {}
                        },
                        Page::Secrets => rsx! { firemage_webui_page_secrets::Secrets {} },
                        Page::Users => rsx! {
                            firemage_webui_page_users::Users {}
                        },
                        Page::Config => rsx! {
                            firemage_webui_page_config::Config {}
                        },
                        Page::Host => rsx! {
                            firemage_webui_page_host::Host {}
                        },
                    }
                }
                footer {
                    span { "FIREMAGE" }
                    span { {concat!("v", env!("CARGO_PKG_VERSION"))} }
                }
            }
        }
    }
}
