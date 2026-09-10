//! Password and OIDC sign-in screen.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_auth::use_auth;
#[component]
pub fn Login() -> Element {
    let auth = use_auth();
    let username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| {
        if auth.session.read()["session_expired"] == true {
            "Your session expired or was revoked. Sign in again to continue.".to_owned()
        } else if firemage_webui_provider_auth::is_oidc_failure() {
            "OpenID Connect sign-in did not complete. Please try again or use your local account."
                .to_owned()
        } else {
            String::new()
        }
    });
    let oidc = auth.session.read()["oidc_enabled"]
        .as_bool()
        .unwrap_or(false);
    rsx! {
        div { class: "login-layout",
            section { class: "login-brand",
                div { class: "brand-symbol",
                    Icon { name: "brand", size: 48 }
                }
                h1 { "Your machines.\nYour control." }
                p { "A clear view of every microVM, from first boot to final output." }
                div { class: "login-decoration",
                    div {}
                    div {}
                    div {}
                }
            }
            section { class: "login-panel",
                div { class: "eyebrow", "FIREMAGE / CONTROL PLANE" }
                h2 { "Welcome back" }
                p { class: "muted", "Sign in to manage this host." }
                Notice { message: error() }
                form {
                    onsubmit: move |e| {
                        e.prevent_default();
                        if busy() {
                            return;
                        }
                        busy.set(true);
                        error.set(String::new());
                        spawn(async move {
                            let result = auth.login(username(), password()).await;
                            password.set(String::new());
                            if let Err(e) = result {
                                error.set(e);
                            }
                            busy.set(false);
                        });
                    },
                    Field {
                        label: "Username",
                        id: "login-username",
                        value: username,
                        required: true,
                    }
                    Field {
                        label: "Password",
                        id: "login-password",
                        value: password,
                        kind: "password",
                        required: true,
                    }
                    button {
                        class: "primary wide",
                        r#type: "submit",
                        disabled: busy(),
                        if busy() {
                            "Signing in…"
                        } else {
                            "Sign in"
                        }
                    }
                }
                if oidc {
                    div { class: "separator", "or" }
                    a { class: "button wide", href: "/v1/browser/oidc/start",
                        "Continue with OpenID Connect"
                    }
                }
                p { class: "login-note", "Access is managed by your administrator." }
            }
        }
    }
}
