use std::path::PathBuf;

use anyhow::Context;
use firemage_wire::{KERNEL_MAX_BYTES, Kernel, KernelAlias, KernelImport, ensure_kernel_name};

#[derive(clap::Subcommand)]
pub enum Command {
    List,
    /// Upload a local file into the server's kernel directory. Requires an administrator.
    Upload {
        file: PathBuf,
        /// Catalog name. Defaults to the local file's basename.
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        alias: String,
    },
    /// Download a kernel on the server and verify its SHA-256. Requires an administrator.
    Download {
        name: String,
        url: String,
        #[arg(long)]
        sha256: String,
        #[arg(long)]
        alias: String,
    },
    /// Set a kernel's unique display alias. Requires an administrator.
    Alias {
        name: String,
        alias: String,
    },
    /// Delete an unused kernel from disk. Requires an administrator.
    Delete {
        name: String,
    },
}

pub async fn run(command: Command, client: &firemage_client::Client) -> anyhow::Result<()> {
    match command {
        Command::List => crate::print(&client.get::<Vec<Kernel>>("/v1/kernels").await?),
        Command::Upload { file, name, alias } => {
            let name = name
                .or_else(|| file.file_name()?.to_str().map(str::to_owned))
                .context("provide --name for a file without a UTF-8 basename")?;
            let path = kernel_path(&name)?;
            KernelAlias {
                alias: Some(alias.clone()),
            }
            .validate()?;
            let mut url = reqwest::Url::parse("https://localhost/")?;
            url.set_path(&format!("{path}/content"));
            url.query_pairs_mut().append_pair("alias", &alias);
            let upload_path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
            let bytes = read_upload(&file)?;
            crate::print(&client.put_bytes::<Kernel>(&upload_path, bytes).await?)
        }
        Command::Download {
            name,
            url,
            sha256,
            alias,
        } => {
            ensure_kernel_name(&name)?;
            KernelAlias {
                alias: Some(alias.clone()),
            }
            .validate()?;
            anyhow::ensure!(
                sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "SHA-256 must contain 64 hexadecimal characters"
            );
            crate::print(
                &client
                    .post::<Kernel>(
                        "/v1/kernels/import",
                        &KernelImport {
                            name,
                            alias,
                            url,
                            sha256,
                        },
                    )
                    .await?,
            )
        }
        Command::Alias { name, alias, .. } => {
            let path = kernel_path(&name)?;
            let update = KernelAlias { alias: Some(alias) };
            update.validate()?;
            crate::print(
                &client
                    .request::<Kernel>("PUT", &path, Some(&update))
                    .await?,
            )
        }
        Command::Delete { name } => crate::print(&client.delete(&kernel_path(&name)?).await?),
    }
}

fn kernel_path(name: &str) -> anyhow::Result<String> {
    ensure_kernel_name(name)?;
    Ok(format!("/v1/kernels/{name}"))
}

pub(super) fn read_upload(path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    crate::upload::read(path, KERNEL_MAX_BYTES, false)
}
