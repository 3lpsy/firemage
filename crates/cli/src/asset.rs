use std::path::PathBuf;

use anyhow::Context;
use firemage_wire::{
    FileAsset, FileAssetAlias, FileAssetLimits, FileAssetUpload, ensure_asset_alias,
    ensure_asset_id,
};

#[derive(clap::Subcommand)]
pub enum Command {
    /// List uploaded files owned by your account.
    List,
    /// Upload a reusable file with an alias unique to your account.
    Upload {
        file: PathBuf,
        #[arg(long)]
        alias: String,
    },
    /// Download a file to a new local path. Existing files are never overwritten.
    Download { id: String, output: PathBuf },
    /// Change a file's alias without changing VM attachments.
    Alias { id: String, alias: String },
    /// Delete an unused file from the server.
    Delete { id: String },
}

pub async fn run(command: Command, client: &firemage_client::Client) -> anyhow::Result<()> {
    match command {
        Command::List => crate::print(&client.get::<Vec<FileAsset>>("/v1/assets").await?),
        Command::Upload { file, alias } => {
            let filename = file
                .file_name()
                .and_then(|name| name.to_str())
                .context("upload file must have a UTF-8 filename")?;
            FileAssetUpload {
                alias: alias.clone(),
                filename: filename.into(),
            }
            .validate()?;
            let mut url = reqwest::Url::parse("http://localhost/v1/assets")?;
            url.query_pairs_mut()
                .append_pair("alias", &alias)
                .append_pair("filename", filename);
            let path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
            let limits: FileAssetLimits = client.get("/v1/assets/limits").await?;
            let bytes = crate::upload::read(&file, limits.max_bytes, true)?;
            crate::print(&client.post_bytes::<FileAsset>(&path, bytes).await?)
        }
        Command::Alias { id, alias } => {
            ensure_asset_alias(&alias)?;
            crate::print(
                &client
                    .request::<FileAsset>("PUT", &asset_path(&id)?, Some(&FileAssetAlias { alias }))
                    .await?,
            )
        }
        Command::Download { id, output } => {
            let limits: FileAssetLimits = client.get("/v1/assets/limits").await?;
            let bytes = client
                .get_bytes(&format!("{}/content", asset_path(&id)?), limits.max_bytes)
                .await?;
            use std::{io::Write, os::unix::fs::OpenOptionsExt};
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&output)
                .with_context(|| format!("creating {}", output.display()))?;
            file.write_all(&bytes)?;
            eprintln!("Saved {} bytes to {}", bytes.len(), output.display());
            Ok(())
        }
        Command::Delete { id } => crate::print(&client.delete(&asset_path(&id)?).await?),
    }
}

fn asset_path(id: &str) -> anyhow::Result<String> {
    ensure_asset_id(id)?;
    Ok(format!("/v1/assets/{id}"))
}
