use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FileAssetLimits {
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileAsset {
    pub id: String,
    pub alias: String,
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub created_at: i64,
    pub vm_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAssetAlias {
    pub alias: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAssetUpload {
    pub alias: String,
    pub filename: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAssetImport {
    pub alias: String,
    pub filename: String,
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
}
impl FileAssetImport {
    pub fn validate(&self) -> anyhow::Result<()> {
        FileAssetUpload {
            alias: self.alias.clone(),
            filename: self.filename.clone(),
        }
        .validate()?;
        anyhow::ensure!(self.url.len() <= 4096, "asset URL is too long");
        let url = url::Url::parse(&self.url)?;
        anyhow::ensure!(
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none(),
            "asset URL must use HTTPS without credentials or fragment"
        );
        if let Some(hash) = &self.sha256 {
            anyhow::ensure!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid asset SHA-256 digest"
            );
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAttachment {
    pub asset_id: String,
    pub destination: String,
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub gid: u32,
    #[serde(default = "default_mode")]
    pub mode: u32,
}
fn default_mode() -> u32 {
    0o644
}
pub fn ensure_asset_id(id: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        id.len() == 36
            && id
                .bytes()
                .enumerate()
                .all(|(i, b)| if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit() && !b.is_ascii_uppercase()
                }),
        "asset ID must be a lowercase UUID"
    );
    Ok(())
}
pub fn ensure_asset_alias(alias: &str) -> anyhow::Result<()> {
    crate::KernelAlias {
        alias: Some(alias.into()),
    }
    .validate()
}
impl FileAssetUpload {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure_asset_alias(&self.alias)?;
        anyhow::ensure!(
            !self.filename.is_empty()
                && self.filename.len() <= 255
                && !self.filename.contains(['/', '\\'])
                && !self.filename.chars().any(char::is_control)
                && self.filename != "."
                && self.filename != "..",
            "asset filename must be a filename without directories or control characters"
        );
        Ok(())
    }
}
impl AssetAttachment {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure_asset_id(&self.asset_id)?;
        crate::BootFile {
            path: "asset".into(),
            content: String::new(),
            encoding: crate::FileEncoding::Utf8,
            destination: Some(self.destination.clone()),
            uid: self.uid,
            gid: self.gid,
            mode: self.mode,
        }
        .validate()
    }
}
