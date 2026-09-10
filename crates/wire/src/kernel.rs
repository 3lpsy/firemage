use serde::{Deserialize, Serialize};

pub const KERNEL_MAX_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Kernel {
    pub name: String,
    pub alias: Option<String>,
    pub size_bytes: u64,
    pub vm_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelImport {
    pub name: String,
    pub url: String,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelAlias {
    pub alias: Option<String>,
}
pub fn ensure_kernel_name(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 128
            && name.as_bytes()[0].is_ascii_alphanumeric()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
        "kernel name must start with a letter or digit and contain at most 128 letters, digits, dots, underscores or hyphens"
    );
    Ok(())
}
impl KernelAlias {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(alias) = &self.alias {
            anyhow::ensure!(
                !alias.trim().is_empty()
                    && alias.len() <= 128
                    && alias.trim() == alias
                    && !alias.chars().any(char::is_control),
                "alias must contain 1-128 characters without surrounding whitespace or control characters"
            );
        }
        Ok(())
    }
}
