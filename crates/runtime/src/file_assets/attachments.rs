use crate::Runtime;
use base64::Engine;
use firemage_wire::{BootFile, FileEncoding, SEED_MAX_BYTES, VmSpec};

impl Runtime {
    pub(crate) async fn validate_asset_attachments(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        let mut total = 0u64;
        for attachment in &spec.attachments {
            attachment.validate()?;
            let row = firemage_queries::file_asset(&self.db, owner, &attachment.asset_id).await?;
            self.file_catalog()?.file(&row.id)?;
            total += row.size_bytes as u64;
        }
        for file in &spec.files {
            total += match file.encoding {
                FileEncoding::Utf8 => file.content.len() as u64,
                FileEncoding::Base64 => (file.content.len() as u64).div_ceil(4) * 3,
            };
        }
        total += spec
            .userdata
            .as_ref()
            .map_or(0, |script| script.len() as u64);
        anyhow::ensure!(
            total <= SEED_MAX_BYTES,
            "combined guest boot inputs exceed 128 MiB"
        );
        Ok(())
    }
    pub(crate) async fn attachment_files(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<Vec<BootFile>> {
        let _guard = self.lock("file-assets").await;
        self.validate_asset_attachments(owner, spec).await?;
        let mut files = Vec::new();
        for (index, attachment) in spec.attachments.iter().enumerate() {
            let bytes = self.file_asset_content(owner, &attachment.asset_id).await?;
            files.push(BootFile {
                path: format!("firemage/attachments/{index}-{}", attachment.asset_id),
                content: base64::engine::general_purpose::STANDARD.encode(bytes),
                encoding: FileEncoding::Base64,
                destination: Some(attachment.destination.clone()),
                uid: attachment.uid,
                gid: attachment.gid,
                mode: attachment.mode,
            });
        }
        Ok(files)
    }
}
