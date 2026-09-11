use crate::Catalog;
use firemage_wire::KernelImport;

pub async fn fetch(
    catalog: &Catalog,
    input: &KernelImport,
) -> anyhow::Result<tempfile::NamedTempFile> {
    firemage_wire::ensure_kernel_name(&input.name)?;
    Ok(catalog
        .directory()
        .download(
            &input.url,
            (!input.sha256.is_empty()).then_some(input.sha256.as_str()),
        )
        .await?
        .file)
}
