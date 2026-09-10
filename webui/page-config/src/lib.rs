//! Validated TOML editing with optimistic revisions and restart information.
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::{get, pretty, request, text};
use firemage_webui_provider_auth::use_auth;
use serde_json::{Value, json};
#[component]
pub fn Config() -> Element {
    let auth = use_auth();
    let mut current = use_signal(|| Value::Null);
    let mut draft = use_signal(String::new);
    let mut review = use_signal(|| None::<Value>);
    let mut message = use_signal(String::new);
    let mut success = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut reload_confirm = use_signal(|| false);
    let mut show_effective = use_signal(|| false);
    use_future(move || async move {
        match get("/v1/config").await {
            Ok(v) => {
                draft.set(text(&v, "toml"));
                current.set(v);
            }
            Err(e) => message.set(e),
        }
    });
    let mut reload = move || {
        busy.set(true);
        spawn(async move {
            match get("/v1/config").await {
                Ok(value) => {
                    draft.set(text(&value, "toml"));
                    current.set(value);
                    review.set(None);
                    success.set(true);
                    message.set("Configuration reloaded.".into());
                }
                Err(error) => {
                    success.set(false);
                    message.set(error);
                }
            }
            busy.set(false);
        });
    };
    let writable = current()["writable"].as_bool().unwrap_or(false);
    let mut save = move |validate: bool| {
        if busy() {
            return;
        }
        let payload = json!({ "toml" : draft(), "revision" : current() ["revision"] });
        busy.set(true);
        message.set(String::new());
        spawn(async move {
            match request(
                if validate { "POST" } else { "PUT" },
                if validate {
                    "/v1/config/validate"
                } else {
                    "/v1/config"
                },
                Some(payload),
                &auth.csrf(),
            )
            .await
            {
                Ok(v) => {
                    success.set(true);
                    if validate {
                        review.set(Some(v));
                        message.set(
                            "Configuration is valid. Review the changes before saving.".into(),
                        );
                    } else {
                        draft.set(text(&v, "toml"));
                        current.set(v);
                        review.set(None);
                        message
                            .set("Configuration saved. Running VMs were not interrupted.".into());
                    }
                }
                Err(e) => {
                    success.set(false);
                    message.set(e);
                }
            }
            busy.set(false);
        });
    };
    rsx! {
        div { class: "heading",
            div {
                div { class: "eyebrow", "ADMINISTRATION" }
                h1 { "Configuration" }
                p { class: "muted", "Server TOML, effective settings, and active overrides." }
            }
            div { class: "actions",
                button {
                    disabled: busy(),
                    onclick: {
                        let mut reload = reload;
                        move |_| {
                            if draft() != text(&current(), "toml") {
                                reload_confirm.set(true);
                            } else {
                                reload();
                            }
                        }
                    },
                    "Reload"
                }
                button {
                    disabled: busy() || !writable,
                    onclick: {
                        let mut save = save;
                        move |_| save(true)
                    },
                    "Validate & review"
                }
            }
        }
        div { class: "policy-guide",
            span { class: "purple",
                Icon { name: "config" }
            }
            p { "CLI options override environment variables. Environment variables override this TOML." }
            Info { title: "Configuration precedence",
                "The editor changes the saved server configuration. Active command-line and environment values keep precedence. Restart-required fields take effect the next time the server starts. Secret placeholders preserve existing credentials unless explicitly replaced."
            }
        }
        Notice { message: message(), success: success() }
        if !current().is_null() && !writable {
            Notice { message: "This server has no writable configuration path. Start it with --config to enable editing." }
        }
        div { class: "config-layout",
            section {
                div { class: "tabs",
                    button {
                        class: if !show_effective() { "active" } else { "" },
                        onclick: move | _ |
                                                        show_effective.set(false),
                        "Server TOML"
                    }
                    button {
                        class: if show_effective() { "active" } else { "" },
                        onclick: move |_| show_effective.set(true),
                        "Effective settings"
                    }
                }
                if show_effective() {
                    h3 { "Active settings" }
                    pre { class: "console config-effective", r#"{pretty(&current()["effective"])}"# }
                    h3 { "After restart" }
                    pre { class: "console config-effective",
                        r#"{pretty(&current()["effective_after_restart"])}"#
                    }
                } else {
                    Editor {
                        label: "Configuration TOML",
                        id: "server-toml",
                        value: draft,
                        rows: 25,
                        disabled: !
                                                        writable,
                    }
                    p { class: "small muted",
                        "Secrets are redacted. Keep their placeholders to preserve the stored values."
                    }
                }
            }
            aside { class: "config-notes",
                h3 { "Deployment settings"
                    Info { title: "Host-only configuration",
                        "Host paths, executables, isolation policy, and authentication provider settings must be changed by the server operator in host TOML, environment variables, or CLI options. The API rejects edits to these fields. Runtime settings such as session lifetime remain editable here."
                    }
                }
                details {
                    summary { "Fields managed on the host" }
                    for value in current()["host_only"].as_array().into_iter().flatten() {
                        div { class: "list-line mono small", "{value.as_str().unwrap_or_default()}" }
                    }
                }
                p { class: "small muted", "Deployment settings require access to the server host." }
                hr {}
                h3 { "Active overrides" }
                if current()["overrides"].as_array().is_none_or(Vec::is_empty) {
                    p { class: "muted small", "No command-line or environment overrides reported." }
                } else {
                    for value in current()["overrides"].as_array().into_iter().flatten() {
                        div { class: "list-line mono small", "{value.as_str().unwrap_or_default()}" }
                    }
                }
                hr {}
                h3 { "Pending restart" }
                if current()["restart_required"].as_array().is_none_or(Vec::is_empty) {
                    p { class: "muted small", "No pending restart changes." }
                } else {
                    for value in current()["restart_required"].as_array().into_iter().flatten() {
                        div { class: "list-line mono small", "{value.as_str().unwrap_or_default()}" }
                    }
                }
                p { class: "muted small", "Saving configuration does not restart the service." }
            }
        }
        if reload_confirm() {
            Confirm {
                title: "Discard unsaved changes?",
                description: "Reloading replaces your draft with the current server configuration.",
                label: "Discard and reload",
                onclose: move |_| reload_confirm.set(false),
                onconfirm: move |_| {
                    reload_confirm.set(false);
                    reload();
                },
            }
        }
        if let Some(value) = review() {
            Modal {
                title: "Review configuration",
                onclose: move |
                                        _ | review.set(None),
                p { class: "muted",
                    "The TOML is valid. Saving uses the current revision to prevent overwriting another administrator's changes."
                }
                h3 { "Proposed TOML" }
                pre { class: "console", {text(&value, "toml")} }
                h3 { "Restart required" }
                if value["restart_required"].as_array().is_none_or(Vec::is_empty) {
                    p { "No restart-required changes reported." }
                } else {
                    ul {
                        for field in value["restart_required"].as_array().into_iter().flatten() {
                            li { class: "mono", "{field.as_str().unwrap_or_default()}" }
                        }
                    }
                }
                div { class: "actions end",
                    button { onclick: move |_| review.set(None), "Keep editing" }
                    button {
                        class: "primary",
                        disabled: busy(),
                        onclick: move |_| save(false),
                        "Save configuration"
                    }
                }
            }
        }
    }
}
