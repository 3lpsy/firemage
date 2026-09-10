use serde_json::{Value, json};
pub struct Form {
    pub name: String,
    pub mode: String,
    pub isolation: String,
    pub socket: String,
    pub kernel: String,
    pub kernel_sha: String,
    pub source: String,
    pub rootfs: String,
    pub rootfs_sha: String,
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
        let vcpus: u8 = self.vcpus.parse().map_err(|_| "vCPUs must be a number")?;
        let memory: u32 = self.memory.parse().map_err(|_| "Memory must be a number")?;
        if !(1..=32).contains(&vcpus) || memory < 64 {
            return Err("Use 1–32 vCPUs and at least 64 MiB memory".into());
        }
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
            if self.kernel.is_empty() || self.rootfs.is_empty() {
                return Err("Kernel and root disk are required".into());
            }
            spec["kernel"] = if self.kernel.starts_with("https://") {
                json!(
                    { "kind" : "remote", "url" : self.kernel, "sha256" : self.kernel_sha
                    }
                )
            } else {
                json!({ "kind" : "local", "path" : self.kernel })
            };
            spec["rootfs"] = match self.source.as_str() {
                "remote" => {
                    json!(
                        { "kind" : "remote", "url" : self.rootfs, "sha256" : self
                        .rootfs_sha }
                    )
                }
                "oci" => {
                    json!({ "kind" : "oci", "image" : self.rootfs, "size_mib" : 2048 })
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
