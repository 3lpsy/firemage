use crate::Runtime;
use firemage_wire::{FileAsset, FileAssetImport};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io::Read};

impl Runtime {
    pub async fn import_file_asset(
        &self,
        owner: &str,
        input: FileAssetImport,
    ) -> anyhow::Result<FileAsset> {
        input.validate()?;
        firemage_queries::user(&self.db, owner).await?;
        {
            let _guard = self.lock("file-assets").await;
            self.ensure_asset_alias_available(owner, &input.alias, None)
                .await?;
        }
        let catalog = self.file_catalog()?;
        let downloaded = catalog
            .download(&input.url, input.sha256.as_deref())
            .await?;
        let _guard = self.lock("file-assets").await;
        self.ensure_asset_alias_available(owner, &input.alias, None)
            .await?;
        let id = uuid::Uuid::new_v4().to_string();
        let row = firemage_orm::file_assets::Model {
            id: id.clone(),
            owner_id: owner.into(),
            alias: input.alias,
            filename: input.filename,
            storage_name: None,
            size_bytes: downloaded.size_bytes as i64,
            sha256: downloaded.sha256,
            created_at: firemage_queries::now(),
        };
        let name = id.clone();
        tokio::task::spawn_blocking(move || catalog.publish(&name, downloaded.file)).await??;
        if let Err(error) = firemage_queries::insert_file_asset(&self.db, row).await {
            let _ = self.file_catalog()?.remove(&id);
            return Err(error);
        }
        self.file_asset(owner, &id).await
    }

    pub async fn refresh_file_assets(&self, owner: &str) -> anyhow::Result<Vec<FileAsset>> {
        let _guard = self.lock("file-assets").await;
        // Only administrators adopt server-added files; existing ownership is preserved.
        if firemage_queries::user(&self.db, owner).await?.admin {
            self.discover_file_assets(owner).await?;
        }
        self.file_assets(owner).await
    }

    async fn discover_file_assets(&self, owner: &str) -> anyhow::Result<()> {
        let catalog = self.file_catalog()?;
        let known: HashSet<_> = firemage_queries::all_file_asset_storage_names(&self.db)
            .await?
            .into_iter()
            .collect();
        let mut aliases: HashSet<_> = firemage_queries::file_assets(&self.db, owner)
            .await?
            .into_iter()
            .map(|row| row.alias)
            .collect();
        for entry in catalog.list()? {
            // UUID storage names belong to uploads, including interrupted writes without metadata.
            if known.contains(&entry.name) || firemage_wire::ensure_asset_id(&entry.name).is_ok() {
                continue;
            }
            let file = catalog.file(&entry.name)?;
            let maximum = self.config.asset_max_bytes();
            let (size, hash) =
                tokio::task::spawn_blocking(move || fingerprint(file, maximum)).await??;
            let alias = crate::catalog_alias::available_alias(&entry.name, &aliases);
            let row = firemage_orm::file_assets::Model {
                id: uuid::Uuid::new_v4().to_string(),
                owner_id: owner.into(),
                alias: alias.clone(),
                filename: entry.name.clone(),
                storage_name: Some(entry.name),
                size_bytes: size as i64,
                sha256: hash,
                created_at: firemage_queries::now(),
            };
            firemage_queries::insert_file_asset(&self.db, row).await?;
            aliases.insert(alias);
        }
        Ok(())
    }
}

fn fingerprint(mut file: std::fs::File, maximum: u64) -> anyhow::Result<(u64, String)> {
    let before = file.metadata()?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        anyhow::ensure!(size <= maximum, "asset exceeds {maximum} bytes");
        hash.update(&buffer[..count]);
    }
    let after = file.metadata()?;
    anyhow::ensure!(
        before.len() == size && after.len() == size && before.modified()? == after.modified()?,
        "server-added asset changed while being read; retry after the copy finishes"
    );
    Ok((size, hex::encode(hash.finalize())))
}
