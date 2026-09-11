use firemage_assets::{RegistryCredentials, RegistryOptions};
use firemage_wire::{Asset, RegistryAccess, RegistryAuth, VmSpec};
use std::path::{Path, PathBuf};

use crate::Runtime;

impl Runtime {
    pub(crate) async fn registry_options(
        &self,
        owner: &str,
        access: Option<&RegistryAccess>,
    ) -> anyhow::Result<RegistryOptions> {
        let Some(access) = access else {
            return Ok(RegistryOptions::default());
        };
        access.validate()?;
        let credentials = match &access.auth {
            Some(RegistryAuth::Basic {
                username,
                password_secret,
            }) => RegistryCredentials::Basic {
                username: username.clone(),
                password: self.registry_secret(owner, password_secret).await?,
            },
            Some(RegistryAuth::Bearer { token_secret }) => {
                let token = self.registry_secret(owner, token_secret).await?;
                RegistryCredentials::Bearer { token }
            }
            None => RegistryCredentials::Anonymous,
        };
        let ca_pem = match &access.ca_secret {
            Some(name) => Some(self.registry_secret(owner, name).await?),
            None => None,
        };
        let options = RegistryOptions {
            credentials,
            token_realm: access.token_realm.clone(),
            ca_pem,
        };
        options.validate()?;
        Ok(options)
    }

    async fn registry_secret(&self, owner: &str, name: &str) -> anyhow::Result<String> {
        self.secrets()
            .await?
            .resolve(owner, name)
            .await
            .map_err(|_| anyhow::anyhow!("referenced registry secret is unavailable"))
    }

    pub(crate) async fn validate_registry_dependencies(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        for asset in spec
            .kernel
            .iter()
            .chain(spec.rootfs.iter())
            .chain(spec.initrd.iter())
            .chain(spec.drives.iter().map(|drive| &drive.asset))
        {
            if let Asset::Oci { registry, .. } = asset {
                self.registry_options(owner, registry.as_ref()).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn materialize_asset(
        &self,
        owner: &str,
        asset: &Asset,
        destination: &Path,
        command_override: Option<&[String]>,
    ) -> anyhow::Result<PathBuf> {
        let registry = if let Asset::Oci { registry, .. } = asset {
            self.registry_options(owner, registry.as_ref()).await?
        } else {
            RegistryOptions::default()
        };
        firemage_assets::materialize_with_registry(asset, destination, &registry, command_override)
            .await
    }
}

#[cfg(test)]
#[path = "registry/tests.rs"]
mod tests;
