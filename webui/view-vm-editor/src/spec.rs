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
                    let mut rootfs =
                        json!({ "kind" : "oci", "image" : self.rootfs, "size_mib" : 2048 });
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
                if key == "rootfs" && field == "size_mib" && previous.contains_key(field) {
                    continue;
                }
                previous.insert(field.clone(), value.clone());
            }
        } else {
            result[key] = next.clone();
        }
    }
    result
}
