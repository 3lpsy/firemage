use super::*;

#[test]
fn limits_preserve_permissions_assets_and_future_fields() {
    let spec = json!({"name":"worker", "vcpus":2, "future":true, "security":{"mode":"jailed"}, "environment":{"KEY":{"secret":"key"}}, "files":[{"path":"input"}], "network":{"network":"restricted"}});
    let mut draft = LimitsDraft::from_spec(&spec);
    draft.0[2] = "125".into();
    let edited = draft.apply(&spec).unwrap();
    assert_eq!(edited["security"]["cpu_percent"], 125);
    assert_eq!(edited["security"]["memory_overhead_mib"], 256);
    assert!(edited["security"]["file_size_mib"].is_null());
    assert_eq!(edited["future"], true);
    for field in ["name", "environment", "files", "network"] {
        assert_eq!(edited[field], spec[field]);
    }
}

#[test]
fn limits_reject_zero_negative_fractional_and_overflow_values() {
    for invalid in ["0", "-1", "1.5", "4294967296"] {
        let mut draft = LimitsDraft::from_spec(&json!({}));
        draft.0[1] = invalid.into();
        assert!(draft.apply(&json!({})).is_err());
    }
}

#[test]
fn existing_limits_require_a_stopped_vm_but_creation_drafts_are_editable() {
    for state in [
        "defined", "stopped", "failed", "ready", "running", "paused", "unknown",
    ] {
        let vm = json!({"state":state});
        assert_eq!(
            super::is_editable(&vm, false),
            matches!(state, "defined" | "stopped" | "failed")
        );
    }
    assert!(super::is_editable(&json!({}), true));
}
