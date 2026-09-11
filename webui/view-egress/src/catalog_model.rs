use firemage_webui_provider_api::text;
use serde_json::{Value, json};

pub fn rows(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}
pub fn usage(value: &Value, kind: &str) -> usize {
    value[format!("{kind}_count")]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| value[format!("{kind}s")].as_array().map_or(0, Vec::len))
}
pub fn running(value: &Value) -> usize {
    value["vms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|vm| matches!(vm["state"].as_str(), Some("running" | "paused")))
        .count()
}
pub fn vm_label(vm: &Value) -> String {
    let name = vm["name"]
        .as_str()
        .or(vm["spec"]["name"].as_str())
        .unwrap_or("VM");
    let id = text(vm, "id");
    format!("{name} ({})", id.chars().take(8).collect::<String>())
}
pub fn policy_input(
    alias: &str,
    mut policy: Value,
    upstream: &str,
    ca: &str,
) -> Result<Value, String> {
    if alias.trim().is_empty() {
        return Err("Enter a unique policy alias.".into());
    }
    let object = policy.as_object_mut().ok_or("Invalid policy")?;
    object.remove("upstream");
    object.insert("inherit_upstream".into(), json!(upstream == "default"));
    if policy["http"].is_object() {
        policy["http"]["port"] = json!(3128);
        if ca.trim().is_empty() {
            policy["http"]
                .as_object_mut()
                .unwrap()
                .remove("upstream_ca_pem");
        } else {
            policy["http"]["upstream_ca_pem"] = json!(ca);
        }
    }
    let parsed: firemage_wire::EgressPolicy =
        serde_json::from_value(policy.clone()).map_err(|e| e.to_string())?;
    parsed.validate().map_err(|e| e.to_string())?;
    Ok(
        json!({"alias":alias.trim(),"policy":policy,"upstream_proxy_id":if upstream.is_empty() || upstream == "default" { Value::Null } else { json!(upstream) }}),
    )
}

#[cfg(test)]
mod tests;
