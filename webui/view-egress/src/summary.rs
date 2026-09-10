use crate::model::string;
use dioxus::prelude::*;
use firemage_webui_component_controls::*;
use firemage_webui_provider_api::text;
use serde_json::Value;

#[component]
pub fn Summary(value: Value) -> Element {
    let enabled = value["enabled"].as_bool().unwrap_or(false);
    let active = value["active"].as_bool().unwrap_or(false);
    let proxy = text(&value, "proxy_url");
    rsx! {
        div { class: "egress-state",
            Status { value: if active { "running" } else if enabled { "configured" } else { "disabled" } }
            span { class: "small muted",
                if active { "Listeners active" } else if enabled { "Configured. Listeners start with the VM." } else { "No egress configured" }
            }
        }
        if !enabled {
            p { class: "muted small", "Add allowed HTTP destinations or fixed TCP tunnels. A Firemage-only network blocks all other guest traffic." }
        }
        if enabled {
            h3 { "Upstream route" }
            p { class: "small mono egress-route",
                if let Some(url) = value["upstream"]["url"].as_str() {
                    "VM → Firemage → {url} → destination"
                } else { "VM → Firemage → destination" }
            }
        }
        if !proxy.is_empty() {
            h3 { "HTTP proxy"
                Info { title: "Set up the guest proxy",
                    p { "The seed provides /firemage/input/firemage/ca.pem and proxy variables in /firemage/input/firemage/environment.sh. OCI guests load these automatically. For custom images, source the environment script and install the CA in the guest’s operating system or application trust store. Set HTTP_PROXY and HTTPS_PROXY to the same HTTP proxy URL. HTTPS requests use CONNECT and are decrypted by Firemage so path, method, and credential rules can be applied." }
                    p { "Applications must use the proxy. A direct connection remains blocked. Certificate-pinned clients may need application-specific trust configuration." }
                }
            }
            pre { class: "console egress-command", "export HTTP_PROXY={proxy}\nexport HTTPS_PROXY={proxy}\nexport http_proxy=$HTTP_PROXY\nexport https_proxy=$HTTPS_PROXY" }
            div { class: "actions wrap",
                a { class: "button", href: "/v1/egress/ca", download: "firemage-ca.pem", "Download guest CA" }
            }
            if let Some(fingerprint) = value["ca_fingerprint"].as_str() {
                p { class: "small muted", "CA SHA-256" }
                code { class: "egress-fingerprint", "{fingerprint}" }
            }
            h3 { class: "egress-section", "Allowed HTTP requests" }
            if value["http_rules"].as_array().is_none_or(Vec::is_empty) {
                p { class: "small muted", "No matching rules. All HTTP requests are denied." }
            }
            for rule in value["http_rules"].as_array().into_iter().flatten() {
                div { class: "egress-summary-rule",
                    strong { class: "mono small", r#"{text(rule, "scheme")}://{text(rule, "host")}:{string(rule, "port")}"# }
                    span { class: "small", r#"{text(rule, "path_prefix")}"# }
                    span { class: "small muted",
                        if string(rule, "methods").is_empty() { "All standard methods" } else { r#"{string(rule, "methods")}"# }
                    }
                    for (name, source) in rule["headers"].as_object().into_iter().flatten() {
                        span { class: "small purple",
                            if let Some(secret) = source["secret"].as_str() { "{name}: secret {secret}" } else { "{name}: configured literal" }
                        }
                    }
                    if let Some(kind) = rule["signing"]["kind"].as_str() {
                        span { class: "small purple", if kind == "aws_sigv4" { "AWS SigV4 signing" } else { "HMAC SHA-256 signing" } }
                    }
                }
            }
        }
        h3 { class: "egress-section", "TCP tunnels" }
        if value["tunnels"].as_array().is_none_or(Vec::is_empty) { p { class: "small muted", "No tunnels configured." } }
        for tunnel in value["tunnels"].as_array().into_iter().flatten() {
            div { class: "egress-summary-rule",
                strong { r#"{text(tunnel, "name")}"# }
                code { r#"{text(tunnel, "endpoint")}"# }
                span { class: "small muted", r#"to {text(tunnel, "target_host")}:{string(tunnel, "target_port")}"# }
            }
        }
    }
}
