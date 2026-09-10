use serde_json::{Value, json};

#[derive(Clone, Default, PartialEq)]
pub struct RegistryForm {
    pub mode: String,
    pub username: String,
    pub password_secret: String,
    pub token_secret: String,
    pub token_realm: String,
    pub ca_secret: String,
}

impl RegistryForm {
    pub fn from_value(value: &Value) -> Self {
        let text =
            |value: &Value, field: &str| value[field].as_str().unwrap_or_default().to_owned();
        Self {
            mode: text(&value["auth"], "kind"),
            username: text(&value["auth"], "username"),
            password_secret: text(&value["auth"], "password_secret"),
            token_secret: text(&value["auth"], "token_secret"),
            token_realm: text(value, "token_realm"),
            ca_secret: text(value, "ca_secret"),
        }
    }

    pub fn value(&self) -> Result<Option<Value>, String> {
        let mut registry = json!({});
        match self.mode.as_str() {
            "" | "anonymous" => {}
            "basic" => {
                if self.username.is_empty()
                    || self.username.len() > 256
                    || !self
                        .username
                        .bytes()
                        .all(|b| b.is_ascii_graphic() && b != b':')
                {
                    return Err("Registry username must contain 1–256 ASCII characters without spaces or a colon.".into());
                }
                ensure_secret(&self.password_secret, "Registry password")?;
                registry["auth"] = json!({"kind":"basic", "username":self.username, "password_secret":self.password_secret});
            }
            "bearer" => {
                ensure_secret(&self.token_secret, "Registry bearer token")?;
                registry["auth"] = json!({"kind":"bearer", "token_secret":self.token_secret});
            }
            _ => return Err("Choose a registry authentication mode.".into()),
        }
        if !self.token_realm.is_empty() {
            if !self.token_realm.starts_with("https://") {
                return Err("Trusted token endpoint must use HTTPS.".into());
            }
            registry["token_realm"] = json!(self.token_realm);
        }
        if !self.ca_secret.is_empty() {
            ensure_secret(&self.ca_secret, "Registry CA")?;
            registry["ca_secret"] = json!(self.ca_secret);
        }
        Ok((registry != json!({})).then_some(registry))
    }
}

fn ensure_secret(name: &str, label: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(format!("{label}: choose a secret with a valid name."));
    }
    Ok(())
}
