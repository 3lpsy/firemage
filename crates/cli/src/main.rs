mod args;
mod asset;
mod commands;
mod credentials;
mod kernel;
mod oidc;
mod secret;
mod upload;
mod vm;
use args::{Cli, Command};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    #[cfg(feature = "tests-infra")]
    if let Command::TestsInfra(options) = cli.command {
        return firemage_tests_infra::run(options);
    }
    let path = cli
        .config
        .clone()
        .unwrap_or_else(firemage_config::default_path);
    let config = firemage_config::read(
        &path,
        cli.config.is_some() && !matches!(&cli.command, Command::Login(_)),
    )?;
    let client_config = cli.client.merge(config.client.clone());
    match cli.command {
        Command::Serve(overrides) => {
            let overrides = *overrides;
            let effective = overrides.clone().merge(config.server);
            firemage_server::serve_managed(effective, Some(path), overrides).await
        }
        Command::User(args::User::Bootstrap { username, server }) => {
            commands::bootstrap(username, server.merge(config.server)).await
        }
        Command::Login(login) => {
            credentials::login(login, false, client_config, config, &path).await
        }
        Command::Authtoken(login) => {
            credentials::login(login, true, client_config, config, &path).await
        }
        command => {
            let client = credentials::client(&client_config, true).await?;
            commands::run(command, &client).await
        }
    }
}
fn print(value: &impl serde::Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn toml_file<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> anyhow::Result<T> {
    Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
}
