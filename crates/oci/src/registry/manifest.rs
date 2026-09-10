use super::{
    Registry,
    download::{read_bounded, verify},
};
use anyhow::{Context, ensure};
use oci_spec::image::{ImageIndex, ImageManifest};

pub(super) const ACCEPT: &str = "application/vnd.oci.image.manifest.v1+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.docker.distribution.manifest.list.v2+json";
const MAX_MANIFEST: u64 = 4 * 1024 * 1024;

pub(crate) fn architecture() -> anyhow::Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        _ => anyhow::bail!("OCI VMs require an x86_64 or aarch64 host"),
    }
}

impl Registry {
    async fn manifest_bytes(&mut self, digest: &str, size: Option<u64>) -> anyhow::Result<Vec<u8>> {
        ensure!(
            size.is_none_or(|value| value <= MAX_MANIFEST),
            "OCI manifest exceeds 4 MiB"
        );
        let response = self.get(self.url("manifests", digest)?, false).await?;
        let bytes = read_bounded(response, size.unwrap_or(MAX_MANIFEST)).await?;
        ensure!(
            size.is_none_or(|value| bytes.len() as u64 == value),
            "OCI manifest size mismatch"
        );
        verify(&bytes, digest)?;
        Ok(bytes)
    }

    pub(crate) async fn manifest(&mut self) -> anyhow::Result<ImageManifest> {
        let digest = self
            .reference
            .digest()
            .context("missing OCI digest")?
            .to_owned();
        let mut bytes = self.manifest_bytes(&digest, None).await?;
        let document: serde_json::Value =
            serde_json::from_slice(&bytes).context("invalid OCI manifest")?;
        ensure!(
            document["schemaVersion"] == 2,
            "unsupported OCI manifest schema"
        );
        if document.get("manifests").is_some() {
            let index: ImageIndex =
                serde_json::from_slice(&bytes).context("invalid OCI image index")?;
            let arch = architecture()?;
            let candidates: Vec<_> = index
                .manifests()
                .iter()
                .filter(|descriptor| {
                    descriptor.platform().as_ref().is_some_and(|platform| {
                        platform.os().to_string() == "linux"
                            && platform.architecture().to_string() == arch
                            && platform
                                .variant()
                                .as_deref()
                                .is_none_or(|v| v.is_empty() || (arch == "arm64" && v == "v8"))
                    })
                })
                .collect();
            ensure!(
                candidates.len() == 1,
                "OCI index must have exactly one compatible Linux platform"
            );
            let selected = candidates[0];
            bytes = self
                .manifest_bytes(selected.digest().as_ref(), Some(selected.size()))
                .await?;
        }
        let manifest: ImageManifest =
            serde_json::from_slice(&bytes).context("invalid OCI image manifest")?;
        ensure!(
            manifest.schema_version() == 2 && manifest.artifact_type().is_none(),
            "unsupported OCI manifest"
        );
        ensure!(
            manifest.layers().len() <= 256,
            "OCI images may have at most 256 layers"
        );
        ensure!(
            matches!(
                manifest.config().media_type().to_string().as_str(),
                "application/vnd.oci.image.config.v1+json"
                    | "application/vnd.docker.container.image.v1+json"
            ),
            "unsupported OCI image configuration type"
        );
        for layer in manifest.layers() {
            ensure!(
                matches!(
                    layer.media_type().to_string().as_str(),
                    "application/vnd.oci.image.layer.v1.tar"
                        | "application/vnd.oci.image.layer.v1.tar+gzip"
                        | "application/vnd.oci.image.layer.v1.tar+zstd"
                        | "application/vnd.docker.image.rootfs.diff.tar.gzip"
                ),
                "unsupported OCI layer media type"
            );
            super::ensure_digest(layer.digest().as_ref())?;
        }
        Ok(manifest)
    }
}
