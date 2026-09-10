use crate::Runtime;
use firemage_wire::{FILE_ASSET_MAX_BYTES, FileAsset, FileAssetUpload, VmSpec};
use sha2::{Digest, Sha256};
use std::io::Read;

impl Runtime {
    pub(crate) fn file_catalog(&self) -> anyhow::Result<firemage_catalog_files::Directory> {
        firemage_catalog_files::Directory::open(&self.config.asset_dir(), 0, FILE_ASSET_MAX_BYTES)
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
        bytes: Vec<u8>,
    ) -> anyhow::Result<FileAsset> {
        input.validate()?;
        anyhow::ensure!(
            bytes.len() as u64 <= FILE_ASSET_MAX_BYTES,
            "asset exceeds 32 MiB"
        );
        let _guard = self.lock("file-assets").await;
        firemage_queries::user(&self.db, owner).await?;
        self.ensure_asset_alias_available(owner, &input.alias, None)
            .await?;
        let row = firemage_orm::file_assets::Model {
            id: uuid::Uuid::new_v4().to_string(),
            owner_id: owner.into(),
            alias: input.alias,
            filename: input.filename,
            size_bytes: bytes.len() as i64,
            sha256: hex::encode(Sha256::digest(&bytes)),
            created_at: firemage_queries::now(),
        };
        let catalog = self.file_catalog()?;
        let id = row.id.clone();
        tokio::task::spawn_blocking(move || catalog.upload(&id, &bytes)).await??;
        let id = row.id.clone();
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
        self.file_catalog()?.remove(id)?;
        firemage_queries::delete_file_asset(&self.db, owner, id).await
    }
    pub async fn file_asset_content(&self, owner: &str, id: &str) -> anyhow::Result<Vec<u8>> {
        let row = firemage_queries::file_asset(&self.db, owner, id).await?;
        let catalog = self.file_catalog()?;
        let file = catalog.file(id)?;
        tokio::task::spawn_blocking(move || {
            let mut bytes = Vec::new();
            file.take(FILE_ASSET_MAX_BYTES + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() as u64 <= FILE_ASSET_MAX_BYTES
                    && bytes.len() as i64 == row.size_bytes
                    && hex::encode(Sha256::digest(&bytes)) == row.sha256,
                "asset contents changed on disk; upload a new asset"
            );
            Ok(bytes)
        })
        .await?
    }
}
