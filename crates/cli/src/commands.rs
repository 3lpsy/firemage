use crate::args::{ApiToken, Command, Network, User};
use firemage_client::Client;
use serde_json::Value;

pub async fn bootstrap(username: String, config: firemage_config::Server) -> anyhow::Result<()> {
    firemage_wire::ensure_name(&username)?;
    tokio::fs::create_dir_all(config.data_dir()).await?;
    let db = firemage_queries::connect(&config.database()).await?;
    anyhow::ensure!(
        firemage_queries::users(&db).await?.is_empty(),
        "bootstrap only works on an empty user database"
    );
    let password = rpassword::prompt_password("New password: ")?;
    let confirm = rpassword::prompt_password("Confirm password: ")?;
    anyhow::ensure!(password == confirm, "passwords do not match");
    let hash = firemage_auth::hash_password(&password)?;
    firemage_queries::bootstrap(&db, username, hash).await?;
    eprintln!("Administrator created");
    Ok(())
}
pub async fn run(command: Command, client: &Client) -> anyhow::Result<()> {
    let value: Value = match command {
        Command::Secret(command) => return crate::secret::run(command, client).await,
        Command::Whoami => client.get("/v1/me").await?,
        Command::Logout => client.post("/v1/auth/logout", &Value::Null).await?,
        Command::Apitoken(command) => match command {
            ApiToken::List => client.get("/v1/apitokens").await?,
            ApiToken::Create { name, expires_at } => {
                client
                    .post(
                        "/v1/apitokens",
                        &firemage_wire::CreateApiToken { name, expires_at },
                    )
                    .await?
            }
            ApiToken::Delete { id } => client.delete(&format!("/v1/apitokens/{id}")).await?,
        },
        Command::User(command) => match command {
            User::List => client.get("/v1/users").await?,
            User::Create {
                username,
                admin,
                oidc_subject,
            } => {
                let password = if oidc_subject.is_none() {
                    Some(rpassword::prompt_password("New user's password: ")?)
                } else {
                    None
                };
                client
                    .post(
                        "/v1/users",
                        &firemage_wire::CreateUser {
                            username,
                            password,
                            admin,
                            oidc_subject,
                        },
                    )
                    .await?
            }
            User::Bootstrap { .. } => unreachable!(),
        },
        Command::Network(command) => match command {
            Network::List => client.get("/v1/networks").await?,
            Network::Create { file } => {
                client
                    .post(
                        "/v1/networks",
                        &crate::toml_file::<firemage_wire::NetworkSpec>(&file)?,
                    )
                    .await?
            }
            Network::Delete { name } => {
                firemage_wire::ensure_name(&name)?;
                client.delete(&format!("/v1/networks/{name}")).await?
            }
        },
        Command::Vm(command) => return crate::vm::run(command, client).await,
        _ => unreachable!(),
    };
    crate::print(&value)
}
