use serde_json::Value;

pub fn controls(vm: &Value, networks: &Value) -> Vec<(&'static str, &'static str, bool)> {
    let state = vm["state"].as_str().unwrap_or_default();
    let spec = &vm["spec"];
    let editable = matches!(state, "defined" | "stopped" | "failed");
    let unrestricted_network = spec["network"].is_null()
        || networks.as_array().is_some_and(|rows| {
            rows.iter().any(|net| {
                net["name"] == spec["network"]["network"]
                    && net["policy"]["mode"] != "firemage-only"
            })
        });
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
            matches!(
                state,
                "ready" | "running" | "paused" | "starting" | "unknown"
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
    }
}
