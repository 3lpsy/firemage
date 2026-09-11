use crate::Runtime;
use firemage_wire::{FileAsset, FileAssetUpload, VmSpec};
use sha2::{Digest, Sha256};
use std::io::Read;

impl Runtime {
    pub(crate) fn file_catalog(&self) -> anyhow::Result<firemage_catalog_files::Directory> {
        firemage_catalog_files::Directory::open(
            &self.config.asset_dir(),
            0,
            self.config.asset_max_bytes(),
        )
    }
    pub async fn file_assets(&self, owner: &str) -> anyhow::Result<Vec<FileAsset>> {
        let rows = firemage_queries::file_assets(&self.db, owner).await?;
        let mut counts = std::collections::HashMap::<String, usize>::new();
        for row in firemage_queries::vms(&self.db, Some(owner)).await? {
            let spec: VmSpec = serde_json::from_str(&row.spec)?;
            let ids: std::collections::HashSet<_> = spec
                .attachments
                .iter()
                .map(|attachment| &attachment.asset_id)
                .collect();
            for id in ids {
                *counts.entry(id.clone()).or_default() += 1;
            }
        }
        Ok(rows
            .into_iter()
            .map(|row| FileAsset {
                vm_count: counts.get(&row.id).copied().unwrap_or_default(),
                id: row.id,
                alias: row.alias,
                filename: row.filename,
                size_bytes: row.size_bytes as u64,
                sha256: row.sha256,
                created_at: row.created_at,
            })
            .collect())
    }
    pub async fn file_asset(&self, owner: &str, id: &str) -> anyhow::Result<FileAsset> {
        firemage_queries::file_asset(&self.db, owner, id).await?;
        self.file_assets(owner)
            .await?
            .into_iter()
            .find(|row| row.id == id)
            .ok_or_else(|| firemage_queries::NotFound("asset").into())
    }
    pub async fn upload_file_asset(
        &self,
        owner: &str,
        input: FileAssetUpload,
        bytes: impl AsRef<[u8]> + Send + 'static,
    ) -> anyhow::Result<FileAsset> {
        input.validate()?;
        anyhow::ensure!(
            bytes.as_ref().len() as u64 <= self.config.asset_max_bytes(),
            "asset exceeds {} bytes",
            self.config.asset_max_bytes()
        );
        let _guard = self.lock("file-assets").await;
        firemage_queries::user(&self.db, owner).await?;
        self.ensure_asset_alias_available(owner, &input.alias, None)
            .await?;
        let size_bytes = bytes.as_ref().len() as i64;
        let catalog = self.file_catalog()?;
        let id = uuid::Uuid::new_v4().to_string();
        let filename = id.clone();
        let sha256 = tokio::task::spawn_blocking(move || {
            let sha256 = hex::encode(Sha256::digest(bytes.as_ref()));
            catalog.upload(&filename, bytes.as_ref())?;
            anyhow::Ok(sha256)
        })
        .await??;
        let row = firemage_orm::file_assets::Model {
            id: id.clone(),
            owner_id: owner.into(),
            alias: input.alias,
            filename: input.filename,
            size_bytes,
            sha256,
            created_at: firemage_queries::now(),
        };
        if let Err(error) = firemage_queries::insert_file_asset(&self.db, row).await {
            let _ = self.file_catalog()?.remove(&id);
            return Err(error);
        }
        self.file_asset(owner, &id).await
    }
    async fn ensure_asset_alias_available(
        &self,
        owner: &str,
        alias: &str,
        current: Option<&str>,
    ) -> anyhow::Result<()> {
        firemage_wire::ensure_asset_alias(alias)?;
        anyhow::ensure!(
            !firemage_queries::file_assets(&self.db, owner)
                .await?
                .iter()
                .any(|row| row.alias == alias && Some(row.id.as_str()) != current),
            "an asset with this alias already exists"
        );
        Ok(())
    }
    pub async fn alias_file_asset(
        &self,
        owner: &str,
        id: &str,
        alias: &str,
    ) -> anyhow::Result<FileAsset> {
        let _guard = self.lock("file-assets").await;
        firemage_queries::file_asset(&self.db, owner, id).await?;
        self.ensure_asset_alias_available(owner, alias, Some(id))
            .await?;
        firemage_queries::alias_file_asset(&self.db, owner, id, alias).await?;
        self.file_asset(owner, id).await
    }
    pub async fn delete_file_asset(&self, owner: &str, id: &str) -> anyhow::Result<()> {
        let _guard = self.lock("file-assets").await;
        let row = self.file_asset(owner, id).await?;
        anyhow::ensure!(
            row.vm_count == 0,
            "asset is referenced by {} VM(s); detach it or delete those VM definitions first",
            row.vm_count
        );
        // A reduced upload limit must not prevent deleting an older, larger file.
        firemage_catalog_files::Directory::open(&self.config.asset_dir(), 0, u64::MAX)?
            .remove(id)?;
        firemage_queries::delete_file_asset(&self.db, owner, id).await
    }
    pub async fn file_asset_content(&self, owner: &str, id: &str) -> anyhow::Result<Vec<u8>> {
        let row = firemage_queries::file_asset(&self.db, owner, id).await?;
        let catalog = self.file_catalog()?;
        let file = catalog.file(id)?;
        let maximum = self.config.asset_max_bytes();
        tokio::task::spawn_blocking(move || {
            let mut bytes = Vec::new();
            file.take(maximum + 1).read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() as u64 <= maximum
                    && bytes.len() as i64 == row.size_bytes
                    && hex::encode(Sha256::digest(&bytes)) == row.sha256,
                "asset contents changed on disk; upload a new asset"
            );
            Ok(bytes)
        })
        .await?
    }
}
