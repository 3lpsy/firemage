use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged, deny_unknown_fields)]
pub enum EnvironmentValue {
    Plain(String),
    Secret { secret: String },
}
pub fn validate_environment(values: &BTreeMap<String, EnvironmentValue>) -> anyhow::Result<()> {
    anyhow::ensure!(
        values.len() <= 256,
        "at most 256 environment variables are allowed"
    );
    for (name, value) in values {
        anyhow::ensure!(
            !name.is_empty()
                && name.len() <= 128
                && name.bytes().enumerate().all(|(i, b)| b == b'_'
                    || b.is_ascii_alphabetic()
                    || (i > 0 && b.is_ascii_digit())),
            "invalid environment variable name"
        );
        match value {
            EnvironmentValue::Plain(value) => anyhow::ensure!(
                value.len() <= 65536 && !value.contains('\0'),
                "invalid environment variable value"
            ),
            EnvironmentValue::Secret { secret } => crate::ensure_name(secret)?,
        }
    }
    Ok(())
}
