use firemage_wire::AssetAttachment;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct AttachmentForm {
    pub asset_id: String,
    pub destination: String,
    pub uid: String,
    pub gid: String,
    pub mode: String,
}
impl Default for AttachmentForm {
    fn default() -> Self {
        Self {
            asset_id: String::new(),
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
            destination: value["destination"].as_str().unwrap_or_default().into(),
            uid: value["uid"].as_u64().unwrap_or(0).to_string(),
            gid: value["gid"].as_u64().unwrap_or(0).to_string(),
            mode: format!("{:04o}", value["mode"].as_u64().unwrap_or(0o644)),
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
        .map(AttachmentForm::from_value)
        .collect()
}
pub fn attachments(forms: &[AttachmentForm]) -> Result<Vec<AssetAttachment>, String> {
    forms
        .iter()
        .enumerate()
        .map(|(i, f)| f.attachment().map_err(|e| format!("Asset {}: {e}", i + 1)))
        .collect()
}
