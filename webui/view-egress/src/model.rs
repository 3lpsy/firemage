use serde_json::{Value, json};

#[derive(Clone, PartialEq)]
pub struct Draft {
    pub http: bool,
    pub port: String,
    pub rules: Vec<Value>,
    pub tunnels: Vec<Value>,
    pub upstream: Value,
    pub inherit_upstream: bool,
    original: Value,
}
impl Draft {
    pub fn new(value: &Value) -> Self {
        Self {
            http: value["http"].is_object(),
            port: value["http"]["port"].as_u64().unwrap_or(3128).to_string(),
            rules: value["http"]["rules"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|rule| {
                    let mut rule = rule.clone();
                    rule["_headers"] = json!(
                        rule["headers"]
                            .as_object()
                            .into_iter()
                            .flatten()
                            .map(|(name, value)| json!({"name":name,"value":value}))
                            .collect::<Vec<_>>()
                    );
                    rule
                })
                .collect(),
            tunnels: value["tunnels"].as_array().cloned().unwrap_or_default(),
            upstream: value["upstream"].clone(),
            inherit_upstream: value["inherit_upstream"].as_bool().unwrap_or(true),
            original: value.clone(),
        }
    }
    pub fn policy(&self) -> Result<Value, String> {
        let mut value = self.spec(&json!({"network": {}}))?["egress"].clone();
        if value.is_null() {
            value = json!({"inherit_upstream": self.inherit_upstream, "tunnels": []});
        }
        Ok(value)
    }
    pub fn spec(&self, original: &Value) -> Result<Value, String> {
        let mut spec = original.clone();
        if !self.http && self.tunnels.is_empty() {
            spec.as_object_mut()
                .ok_or("Invalid VM definition")?
                .remove("egress");
            return Ok(spec);
        }
        if !original["network"].is_object() {
            return Err("Attach a Firemage-only network before configuring egress.".into());
        }
        let mut egress = if self.original.is_object() {
            self.original.clone()
        } else {
            json!({})
        };
        if self.http {
            let mut http = if egress["http"].is_object() {
                egress["http"].clone()
            } else {
                json!({})
            };
            http["port"] = json!(port(&self.port, "Proxy port")?);
            http["rules"] = json!(self.rules.iter().map(rule).collect::<Result<Vec<_>, _>>()?);
            egress["http"] = http;
        } else {
            egress.as_object_mut().unwrap().remove("http");
        }
        egress["tunnels"] = json!(
            self.tunnels
                .iter()
                .map(tunnel)
                .collect::<Result<Vec<_>, _>>()?
        );
        if self.upstream.is_object() {
            for name in ["username", "password"] {
                ensure_secret(&self.upstream[name], &format!("upstream proxy {name}"))?;
            }
            egress["upstream"] = clean_optional_sources(&self.upstream);
        } else {
            egress.as_object_mut().unwrap().remove("upstream");
        }
        egress["inherit_upstream"] = json!(self.inherit_upstream);
        spec["egress"] = egress;
        Ok(spec)
    }
}
pub fn string(value: &Value, key: &str) -> String {
    match &value[key] {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Array(value) => value
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}
fn port(value: &str, label: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{label} must be between 1 and 65535."))
}
fn list(value: &Value, key: &str) -> Value {
    json!(
        string(value, key)
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>()
    )
}
fn rule(value: &Value) -> Result<Value, String> {
    let mut rule = value.clone();
    if string(value, "host").trim().is_empty() {
        return Err("Every HTTP rule needs a host.".into());
    }
    rule["host"] = json!(string(value, "host").trim());
    rule["port"] = json!(port(&string(value, "port"), "Upstream HTTP port")?);
    rule["methods"] = list(value, "methods");
    rule["allowed_ips"] = list(value, "allowed_ips");
    if !string(value, "path_prefix").starts_with('/') {
        return Err("HTTP path prefixes must start with /.".into());
    }
    if let Some(headers) = value["_headers"].as_array() {
        let mut values = serde_json::Map::new();
        for header in headers {
            let name = string(header, "name");
            if name.is_empty() {
                return Err("Injected headers need a name.".into());
            }
            ensure_secret(&header["value"], &format!("injected header {name}"))?;
            if values.insert(name, header["value"].clone()).is_some() {
                return Err("Injected header names must be unique.".into());
            }
        }
        rule["headers"] = Value::Object(values);
        rule.as_object_mut().unwrap().remove("_headers");
    }
    if rule["signing"].is_null() {
        rule.as_object_mut().unwrap().remove("signing");
    } else {
        for (name, label) in [
            ("key", "HMAC key"),
            ("access_key", "AWS access key"),
            ("secret_key", "AWS secret key"),
            ("session_token", "AWS session token"),
        ] {
            ensure_secret(&rule["signing"][name], label)?;
        }
        rule["signing"] = clean_optional_sources(&rule["signing"]);
    }
    Ok(rule)
}
fn tunnel(value: &Value) -> Result<Value, String> {
    let mut tunnel = value.clone();
    if string(value, "name").trim().is_empty() || string(value, "target_host").trim().is_empty() {
        return Err("Every TCP tunnel needs a name and destination host.".into());
    }
    tunnel["listen_port"] = json!(port(
        &string(value, "listen_port"),
        "Guest-facing TCP port"
    )?);
    tunnel["target_port"] = json!(port(&string(value, "target_port"), "Destination TCP port")?);
    tunnel["allowed_ips"] = list(value, "allowed_ips");
    Ok(tunnel)
}
pub fn new_rule() -> Value {
    json!({"host":"", "port":443, "scheme":"https", "methods":["GET"], "path_prefix":"/", "allowed_ips":[]})
}
pub fn new_tunnel() -> Value {
    json!({"name":"", "listen_port":15432, "target_host":"", "target_port":5432, "allowed_ips":[]})
}

pub fn openai_rule() -> Value {
    json!({"host":"api.openai.com","port":443,"scheme":"https","methods":["POST"],"path_prefix":"/v1/","allowed_ips":[],"_headers":[{"name":"Authorization","value":{"secret":"","prefix":"Bearer "}}]})
}
fn clean_optional_sources(value: &Value) -> Value {
    let mut value = value.clone();
    for name in ["username", "password", "session_token"] {
        if value[name].is_null() || value[name].as_str() == Some("") {
            value.as_object_mut().unwrap().remove(name);
        }
    }
    value
}

fn ensure_secret(value: &Value, label: &str) -> Result<(), String> {
    if value.is_object() && value["secret"].as_str().is_none_or(str::is_empty) {
        return Err(format!("Choose a secret for {label}."));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
