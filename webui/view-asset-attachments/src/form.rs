use firemage_wire::{AssetAttachment, SecretAttachment};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct AttachmentForm {
    pub asset_id: String,
    pub secret: Option<String>,
    pub destination: String,
    pub uid: String,
    pub gid: String,
    pub mode: String,
}
impl Default for AttachmentForm {
    fn default() -> Self {
        Self {
            asset_id: String::new(),
            secret: None,
            destination: String::new(),
            uid: "0".into(),
            gid: "0".into(),
            mode: "0644".into(),
        }
    }
}
impl AttachmentForm {
    pub fn from_value(value: &Value) -> Self {
        Self {
            asset_id: value["asset_id"].as_str().unwrap_or_default().into(),
            secret: value["secret"].as_str().map(str::to_owned),
            destination: value["destination"].as_str().unwrap_or_default().into(),
            uid: value["uid"].as_u64().unwrap_or(0).to_string(),
            gid: value["gid"].as_u64().unwrap_or(0).to_string(),
            mode: format!(
                "{:04o}",
                value["mode"]
                    .as_u64()
                    .unwrap_or(if value["secret"].is_string() {
                        0o600
                    } else {
                        0o644
                    })
            ),
        }
    }
    pub fn attachment(&self) -> Result<AssetAttachment, String> {
        let value = AssetAttachment {
            asset_id: self.asset_id.clone(),
            destination: self.destination.clone(),
            uid: self
                .uid
                .parse()
                .map_err(|_| "Asset UID must be a non-negative integer")?,
            gid: self
                .gid
                .parse()
                .map_err(|_| "Asset GID must be a non-negative integer")?,
            mode: u32::from_str_radix(&self.mode, 8)
                .map_err(|_| "Asset permissions must be octal, such as 0644")?,
        };
        value.validate().map_err(|e| e.to_string())?;
        Ok(value)
    }
}
pub fn from_spec(spec: &Value) -> Vec<AttachmentForm> {
    spec["attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(spec["secret_attachments"].as_array().into_iter().flatten())
        .map(AttachmentForm::from_value)
        .collect()
}
pub fn attachments(forms: &[AttachmentForm]) -> Result<Vec<AssetAttachment>, String> {
    forms
        .iter()
        .filter(|form| form.secret.is_none())
        .enumerate()
        .map(|(i, f)| f.attachment().map_err(|e| format!("Asset {}: {e}", i + 1)))
        .collect()
}

pub fn secret_attachments(forms: &[AttachmentForm]) -> Result<Vec<SecretAttachment>, String> {
    forms
        .iter()
        .filter_map(|form| form.secret.as_ref().map(|name| (form, name)))
        .map(|(form, name)| {
            let attachment = SecretAttachment {
                secret: name.clone(),
                destination: form.destination.clone(),
                uid: form
                    .uid
                    .parse()
                    .map_err(|_| "Secret file UID must be a non-negative integer")?,
                gid: form
                    .gid
                    .parse()
                    .map_err(|_| "Secret file GID must be a non-negative integer")?,
                mode: u32::from_str_radix(&form.mode, 8)
                    .map_err(|_| "Secret file permissions must be octal, such as 0600")?,
            };
            attachment.validate().map_err(|error| error.to_string())?;
            Ok(attachment)
        })
        .collect()
}
