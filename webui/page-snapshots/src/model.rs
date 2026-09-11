use firemage_webui_provider_api::{get, text};
use serde_json::Value;

pub async fn snapshots() -> Result<Vec<Value>, String> {
    let value = get("/v1/snapshots").await?;
    serde_json::from_value::<Vec<firemage_wire::Snapshot>>(value.clone())
        .map_err(|error| format!("Could not read snapshots: {error}"))?;
    Ok(value.as_array().cloned().unwrap_or_default())
}
pub async fn vms() -> Result<Vec<Value>, String> {
    let value = get("/v1/vms").await?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "Could not read VMs".into())
}
pub fn is_capture_target(vm: &Value) -> bool {
    text(vm, "state") == "paused" && is_managed_jailed(vm)
}
pub fn is_source_vm(vm: &Value, snapshot: &Value) -> bool {
    snapshot["source_vm_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty() && vm["id"].as_str() == Some(id))
}
pub fn snapshot_choice(snapshot: &Value) -> String {
    format!("{} ({})", text(snapshot, "alias"), text(snapshot, "id"))
}
pub fn vm_choice(vm: &Value) -> String {
    format!("{} ({})", vm_name(vm), text(vm, "id"))
}
pub fn vm_name(vm: &Value) -> String {
    text(&vm["spec"], "name")
}
pub fn is_restore_target(vm: &Value, snapshot: &Value) -> bool {
    matches!(text(vm, "state").as_str(), "stopped" | "defined" | "failed")
        && is_managed_jailed(vm)
        && vm["spec"]["vcpus"] == snapshot["vcpus"]
        && vm["spec"]["memory_mib"] == snapshot["memory_mib"]
}
fn is_managed_jailed(vm: &Value) -> bool {
    vm["spec"]["security"]["mode"] == "jailed" && vm["spec"]["socket"].is_null()
}
pub fn size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!(r#"{:.1} GiB"#, bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!(r#"{:.1} MiB"#, bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!(r#"{:.1} KiB"#, bytes as f64 / 1024.0)
    } else {
        format!(r#"{bytes} B"#)
    }
}
