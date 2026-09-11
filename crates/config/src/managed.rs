use crate::{Config, Server};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Mutex};

const REDACTED: &str = "<redacted>";
const MUTABLE_FIELDS: &[&str] = &["session_ttl", "public_url", "egress_upstream"];
#[derive(Debug, Serialize)]
pub struct ConfigView {
    pub toml: String,
    pub revision: String,
    pub effective: serde_json::Value,
    pub effective_after_restart: serde_json::Value,
    pub overrides: Vec<String>,
    pub restart_required: Vec<String>,
    pub writable: bool,
    pub host_only: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigEdit {
    pub toml: String,
    pub revision: String,
}
pub struct ManagedConfig {
    path: Option<PathBuf>,
    overrides: Server,
    startup: Server,
    current: Mutex<Server>,
}
#[derive(Debug)]
pub struct RevisionConflict;
impl std::fmt::Display for RevisionConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("configuration changed; reload before saving")
    }
}
impl std::error::Error for RevisionConflict {}
impl ManagedConfig {
    pub fn new(path: Option<PathBuf>, overrides: Server, effective: Server) -> Self {
        Self {
            path,
            overrides,
            startup: effective.clone(),
            current: Mutex::new(effective),
        }
    }
    pub fn snapshot(&self) -> Server {
        self.current.lock().expect("configuration lock").clone()
    }
    fn source(&self) -> anyhow::Result<String> {
        match &self.path {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(text) => Ok(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok("[server]\n".into())
                }
                Err(error) => Err(error.into()),
            },
            None => Ok(toml::to_string(&Config {
                server: self.startup.clone(),
                ..Default::default()
            })?),
        }
    }
    pub fn view(&self) -> anyhow::Result<ConfigView> {
        let current = self.current.lock().expect("configuration lock");
        self.describe(&self.source()?, &current)
    }
    fn describe(&self, source: &str, current: &Server) -> anyhow::Result<ConfigView> {
        let config: Config =
            toml::from_str(source).map_err(|_| anyhow::anyhow!("invalid configuration TOML"))?;
        let resolved = self.overrides.clone().merge(config.server);
        let startup = normalized(&self.startup)?;
        let mut pending = normalized(&resolved)?;
        let restart_required = pending
            .as_object()
            .expect("server object")
            .iter()
            .filter(|(key, value)| {
                key.as_str() != "session_ttl" && startup.get(key.as_str()) != Some(*value)
            })
            .map(|(key, _)| key.clone())
            .collect();
        let overrides = serde_json::to_value(&self.overrides)?
            .as_object()
            .expect("server object")
            .iter()
            .filter(|(_, value)| !value.is_null())
            .map(|(key, _)| key.clone())
            .collect();
        let mut document: toml::Value =
            toml::from_str(source).map_err(|_| anyhow::anyhow!("invalid configuration TOML"))?;
        redact(&mut document);
        let mut effective = normalized(current)?;
        if !effective["oidc_client_secret"].is_null() {
            effective["oidc_client_secret"] = REDACTED.into();
        }
        if !pending["oidc_client_secret"].is_null() {
            pending["oidc_client_secret"] = REDACTED.into();
        }
        Ok(ConfigView {
            toml: toml::to_string_pretty(&document)?,
            revision: format!("{:x}", Sha256::digest(source)),
            effective,
            effective_after_restart: pending,
            overrides,
            restart_required,
            writable: self.path.is_some(),
            host_only: startup
                .as_object()
                .expect("server object")
                .keys()
                .filter(|field| !MUTABLE_FIELDS.contains(&field.as_str()))
                .cloned()
                .collect(),
        })
    }
    pub fn edit(&self, input: ConfigEdit, save: bool) -> anyhow::Result<ConfigView> {
        anyhow::ensure!(
            input.toml.len() <= 1024 * 1024,
            "configuration exceeds 1 MiB"
        );
        let mut current = self.current.lock().expect("configuration lock");
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("configuration has no writable file"))?;
        let _lock = if save {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            use std::os::unix::fs::OpenOptionsExt;
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(path.with_extension("lock"))?;
            lock.try_lock()
                .map_err(|_| anyhow::anyhow!("configuration is being edited; retry"))?;
            Some(lock)
        } else {
            None
        };
        let source = self.source()?;
        if input.revision != format!("{:x}", Sha256::digest(&source)) {
            return Err(RevisionConflict.into());
        }
        let original: toml::Value = toml::from_str(&source)
            .map_err(|_| anyhow::anyhow!("invalid existing configuration"))?;
        let mut next: toml::Value = toml::from_str(&input.toml)
            .map_err(|_| anyhow::anyhow!("invalid configuration TOML"))?;
        preserve(&mut next, &original)?;
        let text = toml::to_string_pretty(&next)?;
        let config: Config = toml::from_str(&text)
            .map_err(|_| anyhow::anyhow!("invalid or unknown configuration field"))?;
        let effective = self.overrides.clone().merge(config.server);
        effective.validate()?;
        // Compare file values before overrides so a masked edit cannot weaken a later restart.
        let previous: Config = toml::from_str(&source)?;
        let following: Config = toml::from_str(&text)?;
        let previous = normalized(&previous.server)?;
        let following = normalized(&following.server)?;
        for (field, value) in previous.as_object().expect("server object") {
            anyhow::ensure!(
                MUTABLE_FIELDS.contains(&field.as_str()) || following[field] == *value,
                "{field} is host-managed; edit it on the server through TOML, environment, or CLI"
            );
        }
        anyhow::ensure!(
            original.get("client") == next.get("client"),
            "client configuration is host-managed and cannot be edited through the API"
        );
        if save {
            crate::write_private(path, text.as_bytes())?;
            current.session_ttl = effective.session_ttl;
        }
        let mut result = self.describe(&text, &current)?;
        if !save {
            result.revision = input.revision;
        }
        Ok(result)
    }
}
fn redact(value: &mut toml::Value) {
    if let Some(table) = value.as_table_mut() {
        for (key, value) in table {
            if matches!(
                key.as_str(),
                "oidc_client_secret" | "authtoken" | "apitoken"
            ) {
                *value = REDACTED.into();
            } else {
                redact(value);
            }
        }
    }
}
fn preserve(next: &mut toml::Value, original: &toml::Value) -> anyhow::Result<()> {
    if let Some(table) = next.as_table_mut() {
        for (key, value) in table {
            if matches!(
                key.as_str(),
                "oidc_client_secret" | "authtoken" | "apitoken"
            ) && value.as_str() == Some(REDACTED)
            {
                *value = original
                    .get(key)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("redacted field has no existing value"))?;
            } else if value.is_table() {
                preserve(
                    value,
                    original
                        .get(key)
                        .unwrap_or(&toml::Value::Table(Default::default())),
                )?;
            }
        }
    }
    Ok(())
}

fn normalized(config: &Server) -> anyhow::Result<serde_json::Value> {
    let mut config = config.clone();
    let directory = config.data_dir();
    config.data_dir = Some(if directory.is_absolute() {
        directory
    } else {
        std::env::current_dir()?.join(directory)
    });
    config.database = Some(config.database());
    config.kernel_dir = Some(config.kernel_dir());
    config.asset_dir = Some(config.asset_dir());
    config.asset_max_bytes = Some(config.asset_max_bytes());
    config.snapshot_dir = Some(config.snapshot_dir());
    config.snapshot_max_bytes = Some(config.snapshot_max_bytes());
    config.session_ttl = Some(config.session_ttl()? as u64);
    if config.unix_socket.is_none() && config.listen.is_none() {
        config.listen = Some("127.0.0.1:8080".into());
    }
    config
        .firecracker
        .get_or_insert_with(|| "firecracker".into());
    config.firecracker_args.get_or_insert_with(Vec::new);
    Ok(serde_json::to_value(config)?)
}
