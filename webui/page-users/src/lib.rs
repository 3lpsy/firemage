//! Administrator user inventory and account editor.
mod editor;
use dioxus::prelude::*;
use editor::UserEditor;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{get, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::Value;
#[component]
pub fn Users() -> Element {
    let auth = use_auth();
    let mut refresh = use_signal(|| 0);
    let mut editor = use_signal(|| None::<Value>);
    let mut deleting = use_signal(String::new);
    let mut error = use_signal(String::new);
    let rows = use_resource(move || {
        let _ = refresh();
        async { get("/v1/users").await }
    });
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "ADMINISTRATION" }
                h1 { "Users" }
                p { class: "muted", "Manage local and OpenID Connect accounts." }
            }
            button {
                class: "primary",
                onclick: move |_| editor.set(Some(Value::Null)),
                "+ Create user"
            }
        }
        Notice { message: error() }
        match rows.read().as_ref() {
            Some(Ok(value)) => rsx! {
                table {
                    thead {
                        tr {
                            th { "USERNAME" }
                            th { "ROLE" }
                            th { "STATUS" }
                            th { "IDENTITY" }
                            th { "" }
                        }
                    }
                    tbody {
                        for row in value.as_array().into_iter().flatten() {
                            tr {
                                td {
                                    strong { r#"{row["username"].as_str().unwrap_or_default()}"# }
                                }
                                td {
                                    if row["admin"] == true {
                                        "Administrator"
                                    } else {
                                        "Member"
                                    }
                                }
                                td {
                                    Status { value: (if
                                                                                        row["disabled"] == true { "disabled" } else { "active" }).to_owned() }
                                }
                                td {
                                    if row["oidc_subject"].is_string() {
                                        "OpenID Connect"
                                    } else {
                                        "Local"
                                    }
                                }
                                td {
                                    div { class: "actions end",
                                        button {
                                            onclick: {
                                                let row = row.clone();
                                                move |_| editor.set(Some(row.clone()))
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "danger subtle",
                                            onclick: {
                                                let id = text(row, "id");
                                                move |_| deleting.set(id.clone())
                                            },
                                            "Delete"
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
                p { class: "muted", "Loading users…" }
            },
        }
        if let Some(initial) = editor() {
            UserEditor {
                initial,
                onclose: move | _ | editor
                                        .set(None),
                onsaved: move |_| {
                    editor.set(None);
                    refresh += 1;
                },
            }
        }
        if !deleting().is_empty() {
            Confirm {
                title: "Delete user?",
                description: "The account and its credentials will be removed. Users that still own resources cannot be deleted.",
                label: "Delete user",
                onclose: move |_| deleting.set(String::new()),
                onconfirm: move |_| {
                    let id = deleting();
                    deleting.set(String::new());
                    spawn(async move {
                        match request("DELETE", &format!("/v1/users/{id}"), None, &auth.csrf()).await
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
