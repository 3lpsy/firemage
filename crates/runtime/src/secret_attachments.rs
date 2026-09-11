use crate::Runtime;
use firemage_wire::{BootFile, FileEncoding, VmSpec};

impl Runtime {
    pub(crate) async fn validate_secret_attachments(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<()> {
        for attachment in &spec.secret_attachments {
            attachment.validate()?;
            firemage_queries::secret(&self.db, owner, &attachment.secret)
                .await
                .map_err(|_| anyhow::anyhow!("referenced secret file is unavailable"))?;
        }
        Ok(())
    }

    pub(crate) async fn secret_attachment_files(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<Vec<BootFile>> {
        let mut files = Vec::new();
        for (index, attachment) in spec.secret_attachments.iter().enumerate() {
            attachment.validate()?;
            let content = self
                .secrets()
                .await?
                .resolve(owner, &attachment.secret)
                .await
                .map_err(|_| anyhow::anyhow!("referenced secret file is unavailable"))?;
            files.push(BootFile {
                path: format!("firemage/secrets/{index}"),
                content,
                encoding: FileEncoding::Utf8,
                destination: Some(attachment.destination.clone()),
                uid: attachment.uid,
                gid: attachment.gid,
                mode: attachment.mode,
            });
        }
        Ok(files)
    }
}

#[cfg(test)]
#[path = "secret_attachment_tests.rs"]
mod tests;
