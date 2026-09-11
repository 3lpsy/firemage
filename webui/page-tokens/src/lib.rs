//! Self-service API token creation, one-time display, and revocation.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{get, request, text, timestamp};
use firemage_webui_provider_auth::use_auth;
use serde_json::json;
#[component]
pub fn Tokens() -> Element {
    let auth = use_auth();
    let mut refresh = use_signal(|| 0);
    let mut creating = use_signal(|| false);
    let mut token = use_signal(String::new);
    let mut deleting = use_signal(String::new);
    let mut error = use_signal(String::new);
    let rows = use_resource(move || {
        let _ = refresh();
        async { get("/v1/apitokens").await }
    });
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "ACCESS" }
                h1 { "API tokens" }
            }
            button { class: "primary", onclick: move |_| creating.set(true), "+ Create API token" }
        }
        div { class: "policy-guide",
            span { class: "purple",
                Icon { name: "tokens" }
            }
            p { "Tokens act as your user and expire at the time you choose." }
            Info { title: "API tokens",
                "API tokens are static credentials with an expiration. Use FIREMAGE_APITOKEN in automation. Browser sign-in uses a separate session cookie. Revoking a token takes effect immediately."
            }
        }
        Notice { message: error() }
        match rows.read().as_ref() {
            Some(Ok(value)) => rsx! {
                if value.as_array().is_none_or(Vec::is_empty) {
                    Empty {
                        title: "No API tokens",
                        description: "Create a token to let an integration act as your account.",
                    }
                } else {
                    table {
                        thead {
                            tr {
                                th { "NAME" }
                                th { "EXPIRES" }
                                th { "TOKEN ID" }
                                th { "" }
                            }
                        }
                        tbody {
                            for row in value.as_array().into_iter().flatten() {
                                tr {
                                    td {
                                        strong { r#"{row["name"].as_str().unwrap_or_default()}"# }
                                    }
                                    td { {timestamp(&row["expires_at"])} }
                                    td { class: "mono small", r#"{row["id"].as_str().unwrap_or_default()}"# }
                                    td {
                                        button {
                                            class: "danger subtle",
                                            onclick: {
                                                let id = text(row, "id");
                                                move |_| deleting.set(id.clone())
                                            },
                                            "Revoke"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! {
                Notice { message: e.clone() }
            },
            None => rsx! {
                p { class: "muted", "Loading API tokens…" }
            },
        }
        if creating() {
            TokenEditor {
                onclose: move |_| creating.set(false),
                oncreated: move |value| {
                    token.set(value);
                    creating.set(false);
                    refresh += 1;
                },
            }
        }
        if !token().is_empty() {
            Modal {
                title: "Your API token",
                onclose: move | _ |
                                        token.set(String::new()),
                p { class: "muted", "Copy this token now. It will not be shown again." }
                label { class: "field", r#for: "created-token",
                    span { "Token" }
                    textarea {
                        id: "created-token",
                        class: "code-editor",
                        readonly: true,
                        rows: 4,
                        value: "{token}",
                    }
                }
                div { class: "actions end",
                    button {
                        class: "primary",
                        onclick: move | _ | token
                                                        .set(String::new()),
                        "I have saved the token"
                    }
                }
            }
        }
        if !deleting().is_empty() {
            Confirm {
                title: "Revoke API token?",
                description: "Any integration using this token will lose access immediately.",
                label: "Revoke token",
                onclose: move |_| deleting.set(String::new()),
                onconfirm: move |_| {
                    let id = deleting();
                    deleting.set(String::new());
                    spawn(async move {
                        match request("DELETE", &format!("/v1/apitokens/{id}"), None, &auth.csrf())
                            .await
                        {
                            Ok(_) => refresh += 1,
                            Err(e) => error.set(e),
                        }
                    });
                },
            }
        }
    }
}
#[component]
fn TokenEditor(onclose: EventHandler<()>, oncreated: EventHandler<String>) -> Element {
    let auth = use_auth();
    let name = use_signal(String::new);
    let mut days = use_signal(|| "30".to_owned());
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: "Create API token", onclose,
            form {
                class: "modal-form",
                onsubmit: move |e| {
                    e.prevent_default();
                    if busy() {
                        return;
                    }
                    let days = days().parse::<i64>().unwrap_or(30);
                    let now = js_sys::Date::now() as i64 / 1000;
                    busy.set(true);
                    spawn(async move {
                        match request(
                                "POST",
                                "/v1/apitokens",
                                Some(json!({ "name" : name(), "expires_at" : now + days * 86400 })),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(v) => oncreated.call(text(&v, "token")),
                            Err(e) => error.set(e),
                        }
                        busy.set(false);
                    });
                },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    Field {
                        label: "Token name",
                        id: "token-name",
                        value: name,
                        required: true,
                        placeholder: "CI runner",
                    }
                    label { class: "field", r#for: "token-expiry",
                        span { "Expires after" }
                        select {
                            id: "token-expiry",
                            value: "{days}",
                            onchange: move |e| days.set(e.value()),
                            for value in ["1", "7", "30", "90", "365"] {
                                option { value, "{value} days" }
                            }
                        }
                    }
                }
                div { class: "actions end",
                    button { r#type: "button", onclick: move | _ |
                                                        onclose.call(()), "Cancel" }
                    button {
                        class: "primary",
                        r#type: "submit",
                        disabled: busy(),
                        "Create token"
                    }
                }
            }
        }
    }
}
