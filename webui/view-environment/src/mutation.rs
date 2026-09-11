use firemage_webui_provider_api::{get, request};
use serde_json::{Map, Value};

#[derive(Clone, PartialEq)]
pub(crate) enum EditorMode {
    Add,
    Edit { name: String, value: Value },
}

pub(crate) enum Change {
    Add(Map<String, Value>),
    Edit {
        name: String,
        previous: Value,
        replacement: (String, Value),
    },
    Delete {
        name: String,
        previous: Value,
    },
}

pub(crate) fn validate_rows(rows: &[(String, Value)]) -> Result<Map<String, Value>, String> {
    let mut environment = Map::new();
    for (name, value) in rows {
        if name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || index > 0 && byte.is_ascii_digit()
            })
        {
            return Err("Environment names must start with a letter or underscore and contain only letters, digits, and underscores.".into());
        }
        if value.is_object() && value["secret"].as_str().is_none_or(str::is_empty) {
            return Err(format!("Choose a secret for {name}."));
        }
        if !value.is_string() && !value.is_object() {
            return Err(format!("Choose a plain value or secret for {name}."));
        }
        if environment.insert(name.clone(), value.clone()).is_some() {
            return Err(format!("Duplicate environment variable: {name}."));
        }
    }
    Ok(environment)
}

pub(crate) fn apply(spec: &Value, change: Change) -> Result<Value, String> {
    let mut spec = spec.clone();
    let mut environment = spec["environment"].as_object().cloned().unwrap_or_default();
    match change {
        Change::Add(values) => {
            if values.is_empty() {
                return Err("Add at least one variable.".into());
            }
            for (name, value) in values {
                if environment.contains_key(&name) {
                    return Err(format!(
                        "{name} already exists. Use its Edit action to change it."
                    ));
                }
                environment.insert(name, value);
            }
        }
        Change::Edit {
            name,
            previous,
            replacement: (next, value),
        } => {
            ensure_unchanged(&environment, &name, &previous)?;
            if next != name && environment.contains_key(&next) {
                return Err(format!("{next} already exists. Choose a different name."));
            }
            environment.remove(&name);
            environment.insert(next, value);
        }
        Change::Delete { name, previous } => {
            ensure_unchanged(&environment, &name, &previous)?;
            environment.remove(&name);
        }
    }
    spec["environment"] = Value::Object(environment);
    Ok(spec)
}

fn ensure_unchanged(
    environment: &Map<String, Value>,
    name: &str,
    previous: &Value,
) -> Result<(), String> {
    if environment.get(name) != Some(previous) {
        return Err(format!(
            "{name} changed or was removed. Refresh the VM and try again."
        ));
    }
    Ok(())
}

pub(crate) async fn save(id: &str, change: Change, csrf: &str) -> Result<(), String> {
    let path = format!("/v1/vms/{id}");
    let current = get(&path).await?;
    if !matches!(
        current["state"].as_str(),
        Some("defined" | "stopped" | "failed")
    ) {
        return Err("Stop the VM before changing its environment.".into());
    }
    let spec = apply(&current["spec"], change)?;
    request("PUT", &path, Some(spec), csrf).await?;
    Ok(())
}
