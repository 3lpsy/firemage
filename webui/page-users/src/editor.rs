use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn UserEditor(initial: Value, onclose: EventHandler<()>, onsaved: EventHandler<()>) -> Element {
    let auth = use_auth();
    let id = initial["id"].as_str().map(str::to_owned);
    let editing = id.is_some();
    let username = use_signal(|| text(&initial, "username"));
    let password = use_signal(String::new);
    let subject = use_signal(|| text(&initial, "oidc_subject"));
    let mut role = use_signal(|| {
        if initial["admin"] == true {
            "admin".to_owned()
        } else {
            "member".into()
        }
    });
    let mut status = use_signal(|| {
        if initial["disabled"] == true {
            "disabled".to_owned()
        } else {
            "active".into()
        }
    });
    let mut error = use_signal(String::new);
    let mut busy = use_signal(|| false);
    rsx! {
        Modal { title: if editing { "Edit user" } else { "Create user" }, onclose,
            form {
                class: "modal-form",
                onsubmit: move |e| {
                    e.prevent_default();
                    if busy() {
                        return;
                    }
                    let mut body = json!({ "username" : username(), "admin" : role() == "admin" });
                    if editing {
                        body["disabled"] = json!(status() == "disabled");
                        body["clear_oidc"] = json!(subject().is_empty());
                    }
                    if !password().is_empty() {
                        body["password"] = json!(password());
                    }
                    if !subject().is_empty() {
                        body["oidc_subject"] = json!(subject());
                    }
                    let path = id
                        .as_ref()
                        .map(|id| format!("/v1/users/{id}"))
                        .unwrap_or("/v1/users".into());
                    busy.set(true);
                    spawn(async move {
                        match request(
                                if editing { "PUT" } else { "POST" },
                                &path,
                                Some(body),
                                &auth.csrf(),
                            )
                            .await
                        {
                            Ok(_) => onsaved.call(()),
                            Err(e) => error.set(e),
                        }
                        busy.set(false);
                    });
                },
                div { class: "modal-form-body",
                    Notice { message: error() }
                    Field {
                        label: "Username",
                        id: "user-username",
                        value: username,
                        required: true,
                    }
                    Field {
                        label: if editing { "New password (leave blank to keep)" } else { "Password for local login" },
                        id: "user-password",
                        value: password,
                        kind: "password",
                    }
                    Field {
                        label: "OIDC subject (optional)",
                        id: "user-subject",
                        value: subject,
                    }
                    p { class: "muted small",
                        "Provide a local password, an OIDC subject, or both. The subject must match the configured identity provider."
                    }
                    fieldset {
                        legend {
                            "Role"
                            Info { title: "User roles",
                                "Administrators manage virtual machines, networks, users, and server configuration. Members can inspect resources they own and manage their own API tokens."
                            }
                        }
                        div { class: "radio-group",
                            for (value, label) in [("member", "Member"), ("admin", "Administrator")] {
                                label {
                                    input {
                                        r#type: "radio",
                                        name: "user-role",
                                        checked: role() == value,
                                        onchange: move | _ | role.set(value
                                                                                    .into()),
                                    }
                                    "{label}"
                                }
                            }
                        }
                    }
                    if editing {
                        fieldset {
                            legend { "Account status" }
                            div { class: "radio-group",
                                for (value, label) in [("active", "Active"), ("disabled", "Disabled")] {
                                    label {
                                        input {
                                            r#type: "radio",
                                            name: "user-status",
                                            checked: status() == value,
                                            onchange: move | _ | status
                                                                                            .set(value.into()),
                                        }
                                        "{label}"
                                    }
                                }
                            }
                        }
                        p { class: "muted small",
                            "Security changes revoke existing sessions and API tokens. The last active administrator is protected."
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
                        "Save user"
                    }
                }
            }
        }
    }
}
