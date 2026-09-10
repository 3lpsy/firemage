use super::Registry;
use anyhow::{Context, ensure};
use oci_spec::image::Descriptor;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub(crate) fn ensure_digest(digest: &str) -> anyhow::Result<()> {
    ensure!(
        digest
            .strip_prefix("sha256:")
            .is_some_and(|hex| hex.len() == 64
                && hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "OCI content must use a lowercase SHA-256 digest"
    );
    Ok(())
}

pub(super) async fn read_bounded(
    mut response: reqwest::Response,
    max: u64,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        response.content_length().is_none_or(|size| size <= max),
        "OCI response exceeds its size limit"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow::anyhow!("OCI response stream failed"))?
    {
        ensure!(
            (bytes.len() as u64).saturating_add(chunk.len() as u64) <= max,
            "OCI response exceeds its size limit"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) fn verify(bytes: &[u8], digest: &str) -> anyhow::Result<()> {
    ensure_digest(digest)?;
    ensure!(
        format!("sha256:{:x}", Sha256::digest(bytes)) == digest,
        "OCI content digest mismatch"
    );
    Ok(())
}

impl Registry {
    pub(crate) async fn blob_bytes(
        &mut self,
        descriptor: &Descriptor,
        max: u64,
    ) -> anyhow::Result<Vec<u8>> {
        ensure!(descriptor.size() <= max, "OCI blob exceeds its size limit");
        let digest = descriptor.digest().to_string();
        let response = self.get(self.url("blobs", &digest)?, true).await?;
        let bytes = read_bounded(response, descriptor.size()).await?;
        ensure!(
            bytes.len() as u64 == descriptor.size(),
            "OCI blob size mismatch"
        );
        verify(&bytes, &digest)?;
        Ok(bytes)
    }

    pub(crate) async fn blob_file(
        &mut self,
        descriptor: &Descriptor,
        path: &Path,
        remaining: &mut u64,
    ) -> anyhow::Result<()> {
        let expected = descriptor.size();
        ensure!(
            expected <= *remaining,
            "OCI download exceeds its size limit"
        );
        let digest = descriptor.digest().to_string();
        let mut response = self.get(self.url("blobs", &digest)?, true).await?;
        ensure!(
            response
                .content_length()
                .is_none_or(|size| size == expected),
            "OCI blob size mismatch"
        );
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .await?;
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| anyhow::anyhow!("OCI blob stream failed"))?
        {
            size = size
                .checked_add(chunk.len() as u64)
                .context("OCI blob length overflow")?;
            ensure!(size <= expected, "OCI blob exceeds its declared size");
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
        }
        ensure!(size == expected, "OCI blob size mismatch");
        ensure!(
            format!("sha256:{:x}", hasher.finalize()) == digest,
            "OCI blob digest mismatch"
        );
        file.flush().await?;
        *remaining -= size;
        Ok(())
    }
}
