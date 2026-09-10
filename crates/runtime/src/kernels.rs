use crate::Runtime;
use anyhow::Context;
use firemage_wire::{Asset, Kernel, KernelAlias, VmSpec};

impl Runtime {
    pub fn kernel_name(&self, asset: &Asset) -> anyhow::Result<String> {
        let name = match asset {
            Asset::Kernel { name } => name.clone(),
            Asset::Local { path } => {
                anyhow::ensure!(
                    path.parent() == Some(self.config.kernel_dir().as_path()),
                    "kernels must be selected from the configured kernel directory; import the kernel first"
                );
                path.file_name()
                    .and_then(|name| name.to_str())
                    .context("invalid kernel filename")?
                    .to_owned()
            }
            _ => anyhow::bail!(
                "import the kernel into the kernel catalog before selecting it for a VM"
            ),
        };
        firemage_wire::ensure_kernel_name(&name)?;
        Ok(name)
    }
    pub(crate) fn normalize_kernel(&self, spec: &mut VmSpec) -> anyhow::Result<()> {
        if let Some(asset) = &spec.kernel {
            let name = self.kernel_name(asset)?;
            firemage_kernels::Catalog::open(&self.config.kernel_dir())?.file(&name)?;
            spec.kernel = Some(Asset::Kernel { name });
        }
        Ok(())
    }
    // Callers hold the shared kernels lock across catalog and VM definition changes.
    pub async fn kernels(&self) -> anyhow::Result<Vec<Kernel>> {
        let mut kernels = firemage_kernels::Catalog::open(&self.config.kernel_dir())?.list()?;
        let aliases = firemage_queries::kernel_aliases(&self.db).await?;
        let mut counts = std::collections::HashMap::<String, usize>::new();
        for row in firemage_queries::vms(&self.db, None).await? {
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            if let Some(name) = spec
                .kernel
                .as_ref()
                .and_then(|asset| self.kernel_name(asset).ok())
            {
                *counts.entry(name).or_default() += 1;
            }
        }
        for kernel in &mut kernels {
            kernel.alias = aliases
                .iter()
                .find(|alias| alias.name == kernel.name)
                .map(|alias| alias.alias.clone());
            kernel.vm_count = counts.get(&kernel.name).copied().unwrap_or_default();
        }
        Ok(kernels)
    }
    pub async fn kernel(&self, name: &str) -> anyhow::Result<Kernel> {
        firemage_wire::ensure_kernel_name(name)?;
        self.kernels()
            .await?
            .into_iter()
            .find(|kernel| kernel.name == name)
            .ok_or_else(|| firemage_queries::NotFound("kernel").into())
    }
    pub async fn alias_kernel(&self, name: &str, alias: &KernelAlias) -> anyhow::Result<Kernel> {
        let _guard = self.lock("kernels").await;
        alias.validate()?;
        self.kernel(name).await?;
        firemage_queries::set_kernel_alias(&self.db, name, alias.alias.as_deref()).await?;
        self.kernel(name).await
    }
    pub async fn delete_kernel(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.lock("kernels").await;
        let kernel = self.kernel(name).await?;
        anyhow::ensure!(
            kernel.vm_count == 0,
            "kernel is referenced by {} VM(s); change or delete those VM definitions first",
            kernel.vm_count
        );
        firemage_kernels::Catalog::open(&self.config.kernel_dir())?.remove(name)?;
        firemage_queries::set_kernel_alias(&self.db, name, None).await?;
        Ok(())
    }
}
