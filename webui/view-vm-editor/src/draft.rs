use crate::{fields::Fields, spec};
use dioxus::prelude::{ReadableExt, WritableExt};
use serde_json::Value;

impl Fields {
    pub fn apply_section(mut self, kind: &str, value: &Value) {
        if kind == "boot" {
            self.userdata
                .set(firemage_webui_provider_api::text(value, "userdata"));
            self.attachments
                .set(firemage_webui_view_asset_attachments::from_spec(value));
        }
    }

    pub fn value(self, base: &Value, is_draft: bool) -> Result<Value, String> {
        let generated = if is_draft {
            self.form().draft()?
        } else {
            self.form().spec()?
        };
        let mut value = spec::merge_guided(base, generated);
        value["attachments"] = serde_json::to_value(
            firemage_webui_view_asset_attachments::attachments(&self.attachments.read())?,
        )
        .map_err(|error| error.to_string())?;
        value["secret_attachments"] = serde_json::to_value(
            firemage_webui_view_asset_attachments::secret_attachments(&self.attachments.read())?,
        )
        .map_err(|error| error.to_string())?;
        Ok(value)
    }
}

pub fn validate(value: Value, original: &Value) -> Result<Value, String> {
    let spec: firemage_wire::VmSpec =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    spec.validate().map_err(|error| error.to_string())?;
    let value = serde_json::to_value(spec).map_err(|error| error.to_string())?;
    if original.is_object()
        && (original["socket"] != value["socket"]
            || original["security"]["mode"] != value["security"]["mode"])
    {
        return Err("Runtime socket and isolation mode are fixed at creation.".into());
    }
    Ok(value)
}
