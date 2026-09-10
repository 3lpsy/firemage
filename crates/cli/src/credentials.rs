use anyhow::Context;
use firemage_config::{Client, Config, write_private};
use firemage_wire::{Login, Token};
use std::{io::Write, path::Path};

pub fn username(value: Option<String>) -> anyhow::Result<String> {
    if let Some(value) = value {
        return Ok(value);
    }
    eprint!("Username: ");
    std::io::stderr().flush()?;
    let mut value = String::new();
    std::io::stdin().read_line(&mut value)?;
    Ok(value.trim().into())
}
pub async fn client(config: &Client, refresh: bool) -> anyhow::Result<firemage_client::Client> {
    anyhow::ensure!(
        config.authtoken.is_none() || config.apitoken.is_none(),
        "choose FIREMAGE_AUTHTOKEN or FIREMAGE_APITOKEN, not both"
    );
    let mut token = config.authtoken.clone().or(config.apitoken.clone());
    let mut saved = None;
    if token.is_none()
        && let Some(path) = &config.auth_token_path
    {
        let credential: Token =
            crate::toml_file(path).context("reading saved session; run firemage login")?;
        token = Some(credential.token.clone());
        saved = Some(credential);
    }
    let base = config.url.as_deref().unwrap_or("http://127.0.0.1:8080");
    let client = firemage_client::Client::with_ca(
        base,
        config.client_socket.as_deref(),
        token,
        config.ca_cert.as_deref(),
    )?;
    if refresh
        && saved
            .as_ref()
            .is_some_and(|s| s.expires_at < firemage_queries::now() + 300)
    {
        use std::os::unix::fs::OpenOptionsExt;
        let path = config
            .auth_token_path
            .as_deref()
            .context("missing token path")?;
        let lock_path = path.with_extension("lock");
        let _lock = tokio::task::spawn_blocking(move || -> anyhow::Result<std::fs::File> {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(lock_path)?;
            file.lock()?;
            Ok(file)
        })
        .await??;
        let latest: Token = crate::toml_file(path)?;
        let credential = if latest.expires_at < firemage_queries::now() + 300 {
            let refreshing = firemage_client::Client::with_ca(
                base,
                config.client_socket.as_deref(),
                Some(latest.token),
                config.ca_cert.as_deref(),
            )?;
            let credential: Token = refreshing
                .post("/v1/auth/refresh", &serde_json::json!({}))
                .await?;
            write_private(path, toml::to_string(&credential)?.as_bytes())?;
            credential
        } else {
            latest
        };
        return firemage_client::Client::with_ca(
            base,
            config.client_socket.as_deref(),
            Some(credential.token),
            config.ca_cert.as_deref(),
        );
    }
    Ok(client)
}
pub async fn login(
    input: crate::args::Login,
    token_only: bool,
    mut settings: Client,
    mut config: Config,
    path: &Path,
) -> anyhow::Result<()> {
    let credential: Token = if input.refresh {
        client(&settings, false)
            .await?
            .post("/v1/auth/refresh", &serde_json::json!({}))
            .await?
    } else {
        let client = firemage_client::Client::with_ca(
            settings.url.as_deref().unwrap_or("http://127.0.0.1:8080"),
            settings.client_socket.as_deref(),
            None,
            settings.ca_cert.as_deref(),
        )?;
        if settings.oidc.unwrap_or(false) {
            crate::oidc::login(&client).await?
        } else {
            anyhow::ensure!(
                !settings.password_stdin.unwrap_or(false) || settings.username.is_some(),
                "password-stdin requires a username option, environment variable or config value"
            );
            let username = username(settings.username.clone())?;
            settings.username = Some(username.clone());
            let password = if settings.password_stdin.unwrap_or(false) {
                use std::io::Read;
                let mut value = String::new();
                std::io::stdin().take(1026).read_to_string(&mut value)?;
                value.trim_end_matches(['\r', '\n']).to_owned()
            } else {
                rpassword::prompt_password("Password: ")?
            };
            client
                .post("/v1/auth/login", &Login { username, password })
                .await?
        }
    };
    if token_only {
        println!("{}", credential.token);
        return Ok(());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let token_path = settings
        .auth_token_path
        .clone()
        .unwrap_or_else(|| path.with_extension("session.toml"));
    write_private(&token_path, toml::to_string(&credential)?.as_bytes())?;
    settings.auth_token_path = Some(std::fs::canonicalize(token_path)?);
    settings
        .url
        .get_or_insert_with(|| "http://127.0.0.1:8080".into());
    settings.authtoken = None;
    settings.apitoken = None;
    config.client = settings;
    write_private(path, toml::to_string_pretty(&config)?.as_bytes())?;
    eprintln!("Logged in. Configuration saved to {}", path.display());
    Ok(())
}
