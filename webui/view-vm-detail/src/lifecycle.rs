use serde_json::Value;

pub fn controls(vm: &Value, networks: &Value) -> Vec<(&'static str, &'static str, bool)> {
    let state = vm["state"].as_str().unwrap_or_default();
    let spec = &vm["spec"];
    let editable = matches!(state, "defined" | "stopped" | "failed");
    let unrestricted_network = spec["network"].is_null()
        || networks
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|net| net["id"] == spec["network"]["network"])
                    .or_else(|| {
                        rows.iter()
                            .find(|net| net["name"] == spec["network"]["network"])
                    })
            })
            .is_some_and(|net| net["policy"]["mode"] != "firemage-only");
    let manual = matches!(
        spec["security"]["mode"].as_str(),
        Some("trusted" | "external")
    ) && unrestricted_network;
    vec![
        ("Start", "start", editable || state == "ready"),
        ("Prepare", "prepare", editable),
        ("Launch", "launch", editable && manual),
        ("Pause", "pause", state == "running"),
        ("Resume", "resume", state == "paused"),
        ("Shut down", "shutdown", state == "running"),
        (
            "Stop",
            "stop",
            !spec["socket"].is_string()
                && matches!(
                    state,
                    "ready" | "running" | "paused" | "starting" | "unknown" | "failed"
                ),
        ),
        ("Refresh", "refresh", true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn start_prepares_while_manual_launch_requires_an_unprepared_unrestricted_vm() {
        let enabled = |state, mode, network, action| {
            controls(
                &json!({"state":state,"spec":{"security":{"mode":mode},"network":network}}),
                &json!([{"name":"review","policy":{"mode":"firemage-only"}}]),
            )
            .into_iter()
            .find(|(_, cmd, _)| *cmd == action)
            .unwrap()
            .2
        };
        for state in ["defined", "stopped", "failed", "ready"] {
            assert!(enabled(state, "jailed", Value::Null, "start"));
            assert!(!enabled(state, "jailed", Value::Null, "launch"));
        }
        assert!(enabled("ready", "jailed", Value::Null, "stop"));
        assert!(enabled("defined", "trusted", Value::Null, "launch"));
        assert!(!enabled("ready", "trusted", Value::Null, "launch"));
        assert!(!enabled(
            "defined",
            "trusted",
            json!({"network":"review"}),
            "launch"
        ));
        assert!(!enabled("paused", "jailed", Value::Null, "start"));
        for state in [
            "defined", "ready", "running", "paused", "stopped", "failed", "unknown",
        ] {
            assert_eq!(
                enabled(state, "jailed", Value::Null, "pause"),
                state == "running"
            );
            assert_eq!(
                enabled(state, "jailed", Value::Null, "shutdown"),
                state == "running"
            );
            assert_eq!(
                enabled(state, "jailed", Value::Null, "resume"),
                state == "paused"
            );
        }
        assert!(enabled("failed", "jailed", Value::Null, "stop"));
        let external = controls(
            &json!({"state":"running", "spec":{"socket":"/run/external.sock", "security":{"mode":"external"}}}),
            &Value::Null,
        );
        assert!(
            !external
                .iter()
                .find(|(_, command, _)| *command == "stop")
                .unwrap()
                .2
        );
        assert!(
            external
                .iter()
                .find(|(_, command, _)| *command == "shutdown")
                .unwrap()
                .2
        );
    }
    #[test]
    fn network_uuid_takes_precedence_over_another_networks_display_name() {
        let selected = "10000000-0000-4000-8000-000000000001";
        let networks = json!([
            {"id":"10000000-0000-4000-8000-000000000002", "name":selected,"policy":{"mode":"unrestricted"}},
            {"id":selected,"name":"review","policy":{"mode":"firemage-only"}}
        ]);
        let actions = controls(
            &json!({"state":"stopped","spec":{"security":{"mode":"trusted"},"network":{"network":selected}}}),
            &networks,
        );
        assert!(
            !actions
                .iter()
                .find(|(_, action, _)| *action == "launch")
                .unwrap()
                .2
        );
    }
}
