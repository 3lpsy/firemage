use dioxus::prelude::*;
use firemage_webui_component_controls::Info;

#[component]
pub fn IsolationMode(mut value: Signal<String>) -> Element {
    rsx! {
        fieldset {
            legend { "Host isolation"
                Info { title: "Host isolation",
                    p { "Jailed runs Firecracker with a private filesystem, a dedicated non-root host identity, seccomp filtering, and cgroup limits. Missing host prerequisites stop the launch." }
                    p { "Trusted host process runs Firecracker directly with the service's host permissions. Only use it for workloads you trust. The server operator must explicitly allow this mode." }
                    p { "Guest root is separate from the Firecracker process's host identity. Network policies, copied boot files, and guest credentials are configured independently. Isolation mode is fixed when you create the VM." }
                }
            }
            div { class: "radio-group",
                for (mode, label) in [("jailed", "Jailed"), ("trusted", "Trusted host process")] {
                    label {
                        input { r#type: "radio", name: "vm-isolation", value: mode, checked: value() == mode,
                            onchange: move |_| value.set(mode.into()),
                        }
                        "{label}"
                    }
                }
            }
        }
        if value() == "trusted" {
            p { class: "small muted", "Requires server opt-in. Firemage will run this VM without jailer isolation or managed host limits." }
        } else {
            p { class: "small muted", "Requires a configured jailer and reserved host identity range. Adjust host limits in Security limits." }
        }
    }
}
