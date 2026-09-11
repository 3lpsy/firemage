use crate::Catalog;
use firemage_wire::KernelImport;

pub async fn fetch(
    catalog: &Catalog,
    input: &KernelImport,
) -> anyhow::Result<tempfile::NamedTempFile> {
    firemage_wire::ensure_kernel_name(&input.name)?;
    anyhow::ensure!(
        input.sha256.len() == 64 && input.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "kernel download requires a SHA-256 digest"
    );
    Ok(catalog
        .directory()
        .download(&input.url, Some(&input.sha256))
        .await?
        .file)
}
