use crate::Runtime;
use anyhow::Context;
use firemage_wire::{Asset, Kernel, KernelAlias, KernelImport, VmSpec};

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
    pub async fn kernels(&self) -> anyhow::Result<Vec<Kernel>> {
        let _guard = self.lock("kernels").await;
        self.kernels_locked().await
    }
    // Callers hold the kernels lock across registration and catalog or VM changes.
    pub(crate) async fn kernels_locked(&self) -> anyhow::Result<Vec<Kernel>> {
        let mut kernels = firemage_kernels::Catalog::open(&self.config.kernel_dir())?.list()?;
        let mut aliases = firemage_queries::kernel_aliases(&self.db).await?;
        let mut used = aliases.iter().map(|row| row.alias.clone()).collect();
        for kernel in &kernels {
            if !aliases.iter().any(|row| row.name == kernel.name) {
                let alias = crate::catalog_alias::available_alias(&kernel.name, &used);
                firemage_queries::set_kernel_alias(&self.db, &kernel.name, Some(&alias)).await?;
                used.insert(alias.clone());
                aliases.push(firemage_orm::kernel_aliases::Model {
                    name: kernel.name.clone(),
                    alias,
                });
            }
        }
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
        let _guard = self.lock("kernels").await;
        self.kernel_locked(name).await
    }
    async fn kernel_locked(&self, name: &str) -> anyhow::Result<Kernel> {
        firemage_wire::ensure_kernel_name(name)?;
        self.kernels_locked()
            .await?
            .into_iter()
            .find(|kernel| kernel.name == name)
            .ok_or_else(|| firemage_queries::NotFound("kernel").into())
    }
    pub async fn upload_kernel(
        &self,
        name: &str,
        alias: &str,
        body: Vec<u8>,
    ) -> anyhow::Result<Kernel> {
        firemage_wire::ensure_kernel_name(name)?;
        let _guard = self.lock("kernels").await;
        self.ensure_kernel_alias_available_locked(name, alias)
            .await?;
        let catalog = firemage_kernels::Catalog::open(&self.config.kernel_dir())?;
        let filename = name.to_owned();
        tokio::task::spawn_blocking(move || catalog.upload(&filename, &body)).await??;
        self.finish_kernel_locked(name, alias).await
    }
    pub async fn import_kernel(&self, input: &KernelImport) -> anyhow::Result<Kernel> {
        firemage_wire::ensure_kernel_name(&input.name)?;
        {
            let _guard = self.lock("kernels").await;
            self.ensure_kernel_alias_available_locked(&input.name, &input.alias)
                .await?;
        }
        let catalog = firemage_kernels::Catalog::open(&self.config.kernel_dir())?;
        let file = firemage_kernels::fetch(&catalog, input).await?;
        let _guard = self.lock("kernels").await;
        self.ensure_kernel_alias_available_locked(&input.name, &input.alias)
            .await?;
        catalog.publish(&input.name, file)?;
        self.finish_kernel_locked(&input.name, &input.alias).await
    }
    async fn ensure_kernel_alias_available_locked(
        &self,
        name: &str,
        alias: &str,
    ) -> anyhow::Result<()> {
        KernelAlias {
            alias: Some(alias.into()),
        }
        .validate()?;
        self.kernels_locked().await?;
        let aliases = firemage_queries::kernel_aliases(&self.db).await?;
        anyhow::ensure!(
            !aliases
                .iter()
                .any(|row| row.alias == alias && row.name != name),
            "kernel alias is already in use"
        );
        Ok(())
    }
    async fn finish_kernel_locked(&self, name: &str, alias: &str) -> anyhow::Result<Kernel> {
        if let Err(error) = firemage_queries::set_kernel_alias(&self.db, name, Some(alias)).await {
            firemage_kernels::Catalog::open(&self.config.kernel_dir())?
                .remove(name)
                .context("failed to remove kernel after alias registration failed")?;
            return Err(error);
        }
        self.kernel_locked(name).await
    }
    pub async fn alias_kernel(&self, name: &str, alias: &KernelAlias) -> anyhow::Result<Kernel> {
        let _guard = self.lock("kernels").await;
        alias.validate()?;
        self.kernel_locked(name).await?;
        let value = alias.alias.as_deref().context("kernel alias is required")?;
        self.ensure_kernel_alias_available_locked(name, value)
            .await?;
        firemage_queries::set_kernel_alias(&self.db, name, Some(value)).await?;
        self.kernel_locked(name).await
    }
    pub async fn delete_kernel(&self, name: &str) -> anyhow::Result<()> {
        let _guard = self.lock("kernels").await;
        let kernel = self.kernel_locked(name).await?;
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
