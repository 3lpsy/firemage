use serde_json::{Value, json};

pub(crate) const LIMITS: [(&str, &str, &str); 5] = [
    ("memory_overhead_mib", "Host memory overhead (MiB)", "256"),
    ("pids_max", "Host process / thread limit", "128"),
    ("cpu_percent", "Host CPU quota (%)", "vCPUs × 100"),
    ("file_size_mib", "Maximum host file size (MiB)", "Automatic"),
    ("open_files", "Open file limit", "256"),
];

pub(crate) fn mode(spec: &Value) -> &str {
    spec["security"]["mode"].as_str().unwrap_or("jailed")
}

pub(crate) fn label(mode: &str) -> &str {
    match mode {
        "jailed" => "Jailed",
        "trusted" => "Trusted host process",
        "external" => "External process",
        _ => "Unknown mode",
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct LimitsDraft(pub [String; 5]);

impl LimitsDraft {
    pub(crate) fn from_spec(spec: &Value) -> Self {
        Self(std::array::from_fn(|index| {
            spec["security"][LIMITS[index].0]
                .as_u64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| match index {
                    2 | 3 => String::new(),
                    _ => LIMITS[index].2.into(),
                })
        }))
    }

    pub(crate) fn apply(&self, spec: &Value) -> Result<Value, String> {
        let mut result = spec.clone();
        if !result["security"].is_object() {
            result["security"] = json!({"mode": mode(spec)});
        }
        for (index, (key, label, _)) in LIMITS.iter().enumerate() {
            let text = self.0[index].trim();
            result["security"][key] = if text.is_empty() && matches!(index, 2 | 3) {
                Value::Null
            } else {
                let value = text
                    .parse::<u64>()
                    .map_err(|_| format!("{label} must be a positive whole number."))?;
                if value == 0 || index != 3 && value > u32::MAX.into() {
                    return Err(format!("{label} is outside the allowed range."));
                }
                json!(value)
            };
        }
        let security: firemage_wire::VmSecurity =
            serde_json::from_value(result["security"].clone())
                .map_err(|error| error.to_string())?;
        let vcpus = spec["vcpus"]
            .as_u64()
            .unwrap_or(1)
            .try_into()
            .map_err(|_| "Invalid VM vCPU count")?;
        security
            .validate(vcpus)
            .map_err(|error| error.to_string())?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
