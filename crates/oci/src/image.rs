use crate::{
    RegistryOptions,
    extract::{self, ExtractionBudget},
    registry::Registry,
};
use anyhow::{Context, ensure};
use oci_spec::image::ImageConfiguration;
use std::path::{Path, PathBuf};

pub struct Image {
    directory: tempfile::TempDir,
    root: PathBuf,
    process: serde_json::Value,
    budget: ExtractionBudget,
}

impl Image {
    pub fn rootfs(&self) -> &Path {
        &self.root
    }
    pub fn process(&self) -> &serde_json::Value {
        &self.process
    }
    pub fn has_file(&self, path: &str) -> anyhow::Result<bool> {
        extract::has_file(&self.root, path)
    }
    /// Apply guest metadata only after all layers and host-generated boot files are ready.
    pub fn finalize(&self) -> anyhow::Result<()> {
        extract::finalize(&self.root, &self.budget)
    }
}

/// A command override is supplied at boot; image defaults remain unchanged in the result.
pub async fn unpack(
    image: &str,
    parent: &Path,
    options: &RegistryOptions,
    max_bytes: u64,
    command_override: Option<&[String]>,
) -> anyhow::Result<Image> {
    ensure!(
        (16 * 1024 * 1024..=32 * 1024 * 1024 * 1024).contains(&max_bytes),
        "invalid OCI extraction size limit"
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(1800),
        unpack_inner(image, parent, options, max_bytes, command_override),
    )
    .await
    .context("OCI image preparation exceeded 30 minutes")?
}

async fn unpack_inner(
    image: &str,
    parent: &Path,
    options: &RegistryOptions,
    max_bytes: u64,
    command_override: Option<&[String]>,
) -> anyhow::Result<Image> {
    let mut registry = Registry::new(image, options)?;
    let manifest = registry.manifest().await?;
    let config = registry
        .blob_bytes(manifest.config(), 4 * 1024 * 1024)
        .await?;
    let config: ImageConfiguration =
        serde_json::from_slice(&config).context("invalid OCI image configuration")?;
    ensure!(
        config.os().to_string() == "linux"
            && config.architecture().to_string() == crate::registry::architecture()?,
        "OCI image does not match the host Linux platform"
    );
    ensure!(
        config.rootfs().typ() == "layers"
            && config.rootfs().diff_ids().len() == manifest.layers().len(),
        "OCI configuration layer digests do not match the manifest"
    );
    let process = crate::process::process(&config, command_override)?;
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .prefix(".firemage-oci-")
        .tempdir_in(parent)?;
    let root = directory.path().join("rootfs");
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(&root)?;
    let mut result = Image {
        directory,
        root,
        process,
        budget: ExtractionBudget::new(max_bytes, 1_000_000),
    };
    let mut remaining = max_bytes;
    for (layer, diff_id) in manifest.layers().iter().zip(config.rootfs().diff_ids()) {
        let archive = result.directory.path().join("layer");
        registry.blob_file(layer, &archive, &mut remaining).await?;
        let media_type = layer.media_type().to_string();
        let diff_id = diff_id.clone();
        // The blocking worker owns the temporary tree, including when its caller is cancelled.
        result = tokio::task::spawn_blocking(move || -> anyhow::Result<Image> {
            extract::apply_layer(
                &result.root,
                &archive,
                &media_type,
                &diff_id,
                &mut result.budget,
            )?;
            std::fs::remove_file(archive)?;
            Ok(result)
        })
        .await
        .context("OCI extraction worker failed")??;
    }
    Ok(result)
}
