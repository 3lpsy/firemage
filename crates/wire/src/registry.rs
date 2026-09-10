use serde::{Deserialize, Serialize};

/// Registry credentials stay in the owner's vault; VM documents carry references only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryAccess {
    pub auth: Option<RegistryAuth>,
    pub token_realm: Option<String>,
    pub ca_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RegistryAuth {
    Basic {
        username: String,
        password_secret: String,
    },
    Bearer {
        token_secret: String,
    },
}

impl RegistryAccess {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(RegistryAuth::Basic { username, .. }) = &self.auth {
            anyhow::ensure!(
                !username.is_empty()
                    && username.len() <= 256
                    && username.bytes().all(|b| b.is_ascii_graphic() && b != b':'),
                "registry username must contain 1-256 printable ASCII bytes without colons"
            );
        }
        for name in self.secret_names() {
            crate::ensure_name(name)?;
        }
        if let Some(realm) = &self.token_realm {
            anyhow::ensure!(
                realm.len() <= 2048 && !realm.chars().any(|c| c.is_whitespace() || c.is_control()),
                "registry token realm is invalid or exceeds 2048 bytes"
            );
            let url = url::Url::parse(realm)
                .map_err(|_| anyhow::anyhow!("invalid registry token realm"))?;
            anyhow::ensure!(
                url.scheme() == "https"
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "registry token realm must be HTTPS without credentials, query or fragment"
            );
        }
        Ok(())
    }

    pub fn secret_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        match &self.auth {
            Some(RegistryAuth::Basic {
                password_secret, ..
            }) => names.push(password_secret.as_str()),
            Some(RegistryAuth::Bearer { token_secret }) => names.push(token_secret.as_str()),
            None => {}
        }
        names.extend(self.ca_secret.as_deref());
        names
    }

    pub fn is_anonymous(&self) -> bool {
        self.auth.is_none() && self.ca_secret.is_none() && self.token_realm.is_none()
    }
}

impl crate::Asset {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Self::Oci {
            image,
            size_mib,
            registry,
        } = self
        {
            anyhow::ensure!(
                (16..=32768).contains(size_mib),
                "OCI disk size must be 16-32768 MiB"
            );
            let (name, digest) = image
                .split_once("@sha256:")
                .ok_or_else(|| anyhow::anyhow!("OCI image must be pinned with @sha256:<digest>"))?;
            anyhow::ensure!(
                !name.is_empty()
                    && image.len() <= 2048
                    && !name.starts_with('-')
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-/:".contains(&b))
                    && !name.contains("://")
                    && digest.len() == 64
                    && digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "invalid digest-pinned OCI image reference"
            );
            if let Some(registry) = registry {
                registry.validate()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "registry/tests.rs"]
mod tests;
