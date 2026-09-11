use crate::Runtime;
use anyhow::Context;
use firemage_wire::{
    Asset, ConfigAttachment, ConfigReference, VmConfigDocument, VmConfigImport, VmConfigPreview,
    VmSpec,
};

impl Runtime {
    pub async fn export_vm_config(&self, owner: &str, id: &str) -> anyhow::Result<String> {
        let _vm = self.lock(id).await;
        let _assets = self.lock("file-assets").await;
        let _kernels = self.lock("kernels").await;
        let row = firemage_queries::vm(&self.db, owner, id).await?;
        let mut spec: VmSpec = serde_json::from_str(&row.spec)?;
        let aliases = firemage_queries::kernel_aliases(&self.db).await?;
        let kernel_alias = if let Some(kernel) = spec.kernel.take() {
            let name = self.kernel_name(&kernel)?;
            Some(
                aliases
                    .iter()
                    .find(|item| item.name == name)
                    .with_context(|| format!("assign an alias to kernel {name} before exporting"))?
                    .alias
                    .clone(),
            )
        } else {
            None
        };
        let mut attachments = Vec::new();
        for attachment in std::mem::take(&mut spec.attachments) {
            let asset = firemage_queries::file_asset(&self.db, owner, &attachment.asset_id).await?;
            attachments.push(ConfigAttachment {
                alias: asset.alias,
                destination: attachment.destination,
                uid: attachment.uid,
                gid: attachment.gid,
                mode: attachment.mode,
            });
        }
        let egress_policy_alias = if let Some(policy_id) = spec.egress_policy.take() {
            Some(
                firemage_queries::egress_policy(&self.db, owner, &policy_id)
                    .await?
                    .alias,
            )
        } else {
            None
        };
        let document = VmConfigDocument {
            version: 1,
            kernel_alias,
            egress_policy_alias,
            attachments,
            vm: spec,
        };
        super::validation::ensure_portable(&document)?;
        Ok(toml::to_string_pretty(&document)?)
    }

    pub async fn resolve_vm_config(
        &self,
        owner: &str,
        input: &VmConfigImport,
        existing: Option<&str>,
    ) -> anyhow::Result<(VmSpec, VmConfigPreview)> {
        anyhow::ensure!(
            input.toml.len() <= 1024 * 1024,
            "VM config import exceeds 1 MiB"
        );
        let mut doc: VmConfigDocument =
            toml::from_str(&input.toml).context("invalid portable VM config")?;
        super::validation::ensure_portable(&doc)?;
        if let Some(name) = &input.name {
            doc.vm.name.clone_from(name);
        }
        if let Some(id) = existing {
            firemage_queries::vm(&self.db, owner, id).await?;
        }
        let mut references = Vec::new();
        if let Some(alias) = &doc.egress_policy_alias {
            let policies = firemage_queries::egress_policies(&self.db, Some(owner)).await?;
            let policy = policies
                .iter()
                .find(|policy| &policy.alias == alias)
                .with_context(|| {
                    format!("egress policy alias {alias:?} was not found in this owner's library")
                })?;
            doc.vm.egress_policy = Some(policy.id.clone());
            references.push(reference("Egress policy", alias));
            if let Some(proxy_id) = &policy.upstream_proxy_id {
                let proxy = firemage_queries::upstream_proxy(&self.db, owner, proxy_id).await?;
                references.push(reference("Upstream proxy", &proxy.alias));
            }
        }
        if let Some(alias) = &doc.kernel_alias {
            let aliases = firemage_queries::kernel_aliases(&self.db).await?;
            let kernel = aliases.iter().find(|item| &item.alias == alias)
                .with_context(|| format!("kernel alias {alias:?} was not found; add it in Kernels or edit this reference"))?;
            doc.vm.kernel = Some(Asset::Kernel {
                name: kernel.name.clone(),
            });
            references.push(reference("Kernel", alias));
        }
        let assets = firemage_queries::file_assets(&self.db, owner).await?;
        for attachment in &doc.attachments {
            let asset = assets
                .iter()
                .find(|asset| asset.alias == attachment.alias)
                .with_context(|| {
                    format!(
                        "asset alias {:?} was not found in this owner's library",
                        attachment.alias
                    )
                })?;
            doc.vm
                .attachments
                .push(attachment.resolve(asset.id.clone()));
            references.push(reference("Asset", &attachment.alias));
        }
        if let Some(net) = &doc.vm.network {
            references.push(reference("Network", &net.network));
            if existing.is_none() {
                doc.vm.network = Some(
                    self.suggest_network_address(owner, &net.network, None)
                        .await?,
                );
            }
        }
        self.normalize_kernel(&mut doc.vm)?;
        self.ensure_isolation_policy(&doc.vm)?;
        self.validate_dependencies(owner, &doc.vm).await?;
        if let Some(net) = &doc.vm.network {
            self.ensure_network_address_available(owner, net, existing)
                .await?;
        }
        let mut secrets = std::collections::BTreeSet::new();
        collect_secret_references(&serde_json::to_value(&doc.vm)?, &mut secrets);
        references.extend(secrets.iter().map(|alias| reference("Secret", alias)));
        let preview = VmConfigPreview {
            name: doc.vm.name.clone(),
            references,
            address: doc.vm.network.as_ref().map(|net| net.address.to_string()),
        };
        Ok((doc.vm, preview))
    }
}
fn reference(kind: &str, alias: &str) -> ConfigReference {
    ConfigReference {
        kind: kind.into(),
        alias: alias.into(),
    }
}
fn collect_secret_references(
    value: &serde_json::Value,
    names: &mut std::collections::BTreeSet<String>,
) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                if (key == "secret" || key.ends_with("_secret"))
                    && let Some(name) = value.as_str()
                {
                    names.insert(name.into());
                } else {
                    collect_secret_references(value, names);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_secret_references(value, names);
            }
        }
        _ => {}
    }
}
