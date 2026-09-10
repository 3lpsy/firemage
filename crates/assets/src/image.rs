use anyhow::Context;
use firemage_wire::Asset;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

// Each VM owns its writable copies; source images are never attached writable.
pub async fn materialize(asset: &Asset, destination: &Path) -> anyhow::Result<PathBuf> {
    if let Asset::Oci {
        registry: Some(registry),
        ..
    } = asset
    {
        anyhow::ensure!(
            registry.is_anonymous(),
            "OCI registry access must be resolved before materialization"
        );
    }
    materialize_with_registry(asset, destination, &crate::RegistryOptions::default()).await
}

/// The caller resolves registry secrets before invoking this entry point.
pub async fn materialize_with_registry(
    asset: &Asset,
    destination: &Path,
    registry: &crate::RegistryOptions,
) -> anyhow::Result<PathBuf> {
    asset.validate()?;
    let partial = destination.with_extension("partial");
    let result = materialize_inner(asset, &partial, registry).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&partial).await;
    }
    result?;
    tokio::fs::rename(&partial, destination).await?;
    Ok(destination.to_path_buf())
}
async fn materialize_inner(
    asset: &Asset,
    destination: &Path,
    registry: &crate::RegistryOptions,
) -> anyhow::Result<()> {
    match asset {
        Asset::Local { path } => {
            anyhow::ensure!(
                path.is_absolute() && path.is_file(),
                "local asset must be an absolute regular file path"
            );
            tokio::fs::copy(path, destination)
                .await
                .context("copying local asset")?;
        }
        Asset::Remote { url, sha256 } => {
            anyhow::ensure!(
                sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "remote asset requires SHA-256 digest"
            );
            let url = reqwest::Url::parse(url)?;
            anyhow::ensure!(
                url.scheme() == "https" && url.username().is_empty() && url.password().is_none(),
                "asset URL must be HTTPS without credentials"
            );
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(600))
                .build()?;
            let mut response = client.get(url).send().await?.error_for_status()?;
            let tmp = destination.with_extension("download");
            let result = async {
                let mut file = tokio::fs::File::create(&tmp).await?;
                let mut hash = Sha256::new();
                let mut size = 0u64;
                while let Some(chunk) = response.chunk().await? {
                    size += chunk.len() as u64;
                    anyhow::ensure!(
                        size <= 32 * 1024 * 1024 * 1024,
                        "remote asset exceeds 32 GiB"
                    );
                    hash.update(&chunk);
                    file.write_all(&chunk).await?;
                }
                anyhow::ensure!(
                    hex::encode(hash.finalize()).eq_ignore_ascii_case(sha256),
                    "remote asset digest mismatch"
                );
                file.sync_all().await?;
                tokio::fs::rename(&tmp, destination).await?;
                anyhow::Ok(())
            }
            .await;
            if result.is_err() {
                let _ = tokio::fs::remove_file(tmp).await;
            }
            result?;
        }
        Asset::Oci {
            image, size_mib, ..
        } => crate::oci::oci(image, *size_mib, destination, registry).await?,
    }
    Ok(())
}
pub async fn ext4(
    directory: &Path,
    destination: &Path,
    size_mib: u64,
    label: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        (16..=32768).contains(&size_mib),
        "disk size must be 16-32768 MiB"
    );
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(destination)
        .await?;
    file.set_len(size_mib * 1024 * 1024).await?;
    crate::command(
        "mkfs.ext4",
        &[
            "-q",
            "-F",
            "-L",
            label,
            "-d",
            directory.to_str().context("non-UTF8 source path")?,
            destination.to_str().context("non-UTF8 disk path")?,
        ],
    )
    .await
}
