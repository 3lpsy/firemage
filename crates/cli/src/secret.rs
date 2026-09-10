use std::io::{IsTerminal, Read};
#[derive(clap::Subcommand)]
pub enum Command {
    List,
    /// Create or replace a secret. Prompts without echo unless --stdin is used.
    Set {
        name: String,
        #[arg(long)]
        stdin: bool,
    },
    Delete {
        name: String,
    },
}
pub async fn run(command: Command, client: &firemage_client::Client) -> anyhow::Result<()> {
    let output: serde_json::Value = match command {
        Command::List => client.get("/v1/secrets").await?,
        Command::Set { name, stdin } => {
            firemage_wire::ensure_name(&name)?;
            let value = if stdin {
                let mut value = String::new();
                std::io::stdin().take(65537).read_to_string(&mut value)?;
                value
            } else {
                anyhow::ensure!(
                    std::io::stdin().is_terminal(),
                    "secret set requires a terminal or --stdin"
                );
                rpassword::prompt_password("Secret value: ")?
            };
            anyhow::ensure!(
                !value.is_empty() && value.len() <= 65536 && !value.contains('\0'),
                "secret must contain 1-65536 bytes without NUL"
            );
            client
                .request(
                    "PUT",
                    &format!("/v1/secrets/{name}"),
                    Some(&firemage_wire::PutSecret { value }),
                )
                .await?
        }
        Command::Delete { name } => {
            firemage_wire::ensure_name(&name)?;
            client.delete(&format!("/v1/secrets/{name}")).await?
        }
    };
    crate::print(&output)
}
