use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ValueSource {
    Literal(String),
    Secret {
        secret: String,
        #[serde(default)]
        prefix: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SigningConfig {
    AwsSigv4 {
        region: String,
        service: String,
        access_key: ValueSource,
        secret_key: ValueSource,
        #[serde(default)]
        session_token: Option<ValueSource>,
    },
    HmacSha256 {
        key: ValueSource,
        header: String,
        #[serde(default)]
        prefix: String,
    },
}
impl ValueSource {
    pub fn secret_name(&self) -> Option<&str> {
        match self {
            Self::Literal(_) => None,
            Self::Secret { secret, .. } => Some(secret),
        }
    }
}
impl SigningConfig {
    pub fn sources(&self) -> Vec<&ValueSource> {
        match self {
            Self::AwsSigv4 {
                access_key,
                secret_key,
                session_token,
                ..
            } => {
                let mut sources = vec![access_key, secret_key];
                sources.extend(session_token.as_ref());
                sources
            }
            Self::HmacSha256 { key, .. } => vec![key],
        }
    }
}
