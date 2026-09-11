use serde_json::{Value, json};
pub struct Form {
    pub name: String,
    pub mode: String,
    pub isolation: String,
    pub socket: String,
    pub kernel: String,
    pub source: String,
    pub rootfs: String,
    pub rootfs_sha: String,
    pub rootfs_size: String,
    pub workload_mode: String,
    pub command: String,
    pub terminal: bool,
    pub metadata: String,
    pub registry: crate::registry::RegistryForm,
    pub vcpus: String,
    pub memory: String,
    pub network: String,
    pub address: String,
    pub mac: String,
    pub userdata: String,
    pub boot_args: String,
}
impl Form {
    pub fn spec(self) -> Result<Value, String> {
        self.build(false)
    }
    pub fn draft(self) -> Result<Value, String> {
        self.build(true)
    }
    fn build(self, is_draft: bool) -> Result<Value, String> {
        let vcpus = self.vcpus.parse::<u8>();
        let memory = self.memory.parse::<u32>();
        if !is_draft
            && (!vcpus.as_ref().is_ok_and(|v| (1..=32).contains(v))
                || !memory.as_ref().is_ok_and(|m| *m >= 64))
        {
            return Err("Use 1–32 vCPUs and at least 64 MiB memory".into());
        }
        let vcpus = vcpus.map_or_else(|_| json!(self.vcpus), |v| json!(v));
        let memory = memory.map_or_else(|_| json!(self.memory), |v| json!(v));
        let mut spec = json!(
            { "name" : self.name, "vcpus" : vcpus, "memory_mib" : memory, "boot_args" :
            self.boot_args, "files" : [], "drives" : [] }
        );
        let isolation = if self.mode == "socket" {
            "external"
        } else {
            self.isolation.as_str()
        };
        if !matches!(isolation, "jailed" | "trusted" | "external")
            || self.mode != "socket" && isolation == "external"
        {
            return Err("Choose a valid host isolation mode.".into());
        }
        spec["security"] = json!({"mode": isolation});
        spec["terminal"] = json!(self.terminal);
        if !self.metadata.trim().is_empty() {
            spec["metadata"] = parse_json(&self.metadata, "Metadata", is_draft)?;
        }
        if self.mode == "socket" {
            spec["socket"] = json!(self.socket);
        } else {
            if !is_draft && (self.kernel.is_empty() || self.rootfs.is_empty()) {
                return Err("Kernel and root disk are required".into());
            }
            if !is_draft && (self.kernel.contains('/') || self.kernel.contains('\\')) {
                return Err("Choose a kernel from the kernel library.".into());
            }
            spec["kernel"] = json!({"kind":"kernel", "name":self.kernel});
            spec["rootfs"] = match self.source.as_str() {
                "remote" => {
                    json!(
                        { "kind" : "remote", "url" : self.rootfs, "sha256" : self
                        .rootfs_sha }
                    )
                }
                "oci" => {
                    if !is_draft {
                        ensure_oci_reference(&self.rootfs)?;
                    }
                    let size = self.rootfs_size.parse::<u64>();
                    if !is_draft && !size.as_ref().is_ok_and(|n| (16..=32768).contains(n)) {
                        return Err("Root disk size must be 16–32768 MiB.".into());
                    }
                    let size = size.map_or_else(|_| json!(self.rootfs_size), |n| json!(n));
                    let mut rootfs = json!({"kind":"oci", "image":self.rootfs, "size_mib":size});
                    spec["workload"] = json!({"mode":self.workload_mode});
                    if !self.command.trim().is_empty() {
                        let command = parse_json(&self.command, "Workload command", is_draft)?;
                        if !is_draft {
                            let workload: firemage_wire::Workload = serde_json::from_value(
                                json!({"mode":self.workload_mode,"command":command}),
                            )
                            .map_err(|_| {
                                "Workload command must be a JSON array of arguments.".to_owned()
                            })?;
                            workload.validate().map_err(|e| e.to_string())?;
                        }
                        spec["workload"]["command"] = command;
                    }
                    if let Some(registry) = self.registry.value()? {
                        rootfs["registry"] = registry;
                    }
                    rootfs
                }
                _ => json!({ "kind" : "local", "path" : self.rootfs }),
            };
        }
        if !self.network.is_empty() {
            spec["network"] = json!(
                { "network" : self.network, "address" : self.address, "mac" : self.mac }
            );
        }
        if !self.userdata.is_empty() {
            spec["userdata"] = json!(self.userdata);
        }
        Ok(spec)
    }
}

fn ensure_oci_reference(image: &str) -> Result<(), String> {
    firemage_wire::Asset::Oci {
        image: image.into(),
        size_mib: 2048,
        registry: None,
    }
    .validate()
    .map_err(|error| error.to_string())
}
/// TOML has no null; omitted optional fields preserve the VM wire defaults.
pub fn to_toml(value: &Value) -> Result<String, String> {
    if let Some(metadata) = value.get("metadata").filter(|metadata| !metadata.is_null()) {
        toml::Value::try_from(metadata).map_err(|_| {
            "Metadata contains values TOML cannot represent, such as JSON null. Keep using Guided setup to preserve them."
                .to_owned()
        })?;
    }
    fn clean(value: &Value) -> Value {
        match value {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, v)| (k.clone(), clean(v)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(values.iter().map(clean).collect()),
            other => other.clone(),
        }
    }
    toml::to_string_pretty(&clean(value)).map_err(|e| e.to_string())
}

/// Replace guided fields while retaining settings edited elsewhere.
pub fn merge_guided(base: &Value, generated: Value) -> Value {
    let mut result = if base.is_object() {
        base.clone()
    } else {
        json!({})
    };
    for key in [
        "name",
        "vcpus",
        "memory_mib",
        "boot_args",
        "userdata",
        "network",
        "socket",
        "kernel",
        "rootfs",
        "security",
        "workload",
        "terminal",
        "metadata",
    ] {
        let Some(next) = generated.get(key) else {
            if matches!(key, "kernel" | "rootfs") && generated.get("socket").is_some() {
                continue;
            }
            result.as_object_mut().unwrap().remove(key);
            continue;
        };
        if matches!(key, "rootfs" | "security")
            && result[key].is_object()
            && (key == "security" || result[key]["kind"] == next["kind"])
        {
            let previous = result[key].as_object_mut().unwrap();
            if key == "rootfs" {
                previous.remove("registry");
            }
            for (field, value) in next.as_object().unwrap() {
                previous.insert(field.clone(), value.clone());
            }
        } else {
            result[key] = next.clone();
        }
    }
    result
}

fn parse_json(text: &str, label: &str, is_draft: bool) -> Result<Value, String> {
    match serde_json::from_str(text) {
        Ok(value) => Ok(value),
        Err(_) if is_draft => Ok(json!(text)),
        Err(error) => Err(format!("{label} is invalid JSON: {error}")),
    }
}
