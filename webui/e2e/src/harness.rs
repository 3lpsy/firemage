use crate::{oidc::MockProvider, process::*};
use anyhow::{Context, Result};
use std::{path::PathBuf, process::Command, time::Duration};
use thirtyfour::prelude::*;

pub const PASSWORD: &str = "browser-test-password-123";
pub struct Harness {
    pub driver: WebDriver,
    pub url: String,
    pub config_path: PathBuf,
    pub token: String,
    pub client: reqwest::Client,
    pub(crate) name: String,
    pub(crate) screenshots: PathBuf,
    _processes: Processes,
    _directory: tempfile::TempDir,
    _provider: Option<MockProvider>,
}
impl Harness {
    pub fn asset(&self, name: &str) -> String {
        self._directory
            .path()
            .join("assets")
            .join(name)
            .display()
            .to_string()
    }
    pub async fn new(name: &str) -> Result<Self> {
        Self::new_mode(name, false, false).await
    }
    pub async fn oidc(name: &str) -> Result<Self> {
        Self::new_mode(name, true, false).await
    }
    pub async fn oidc_confidential(name: &str) -> Result<Self> {
        Self::new_mode(name, true, true).await
    }
    async fn new_mode(name: &str, with_oidc: bool, confidential: bool) -> Result<Self> {
        let binary = PathBuf::from(
            std::env::var("FIREMAGE_E2E_BINARY").context("run through just test-e2e")?,
        );
        anyhow::ensure!(
            binary.is_absolute() && binary.is_file(),
            "FIREMAGE_E2E_BINARY must name an existing absolute executable"
        );
        let screenshots = PathBuf::from(
            std::env::var("FIREMAGE_E2E_SCREENSHOT_DIR")
                .context("screenshot directory is required")?,
        );
        std::fs::create_dir_all(&screenshots)?;
        let screenshots = screenshots.canonicalize()?;
        let directory = tempfile::Builder::new().prefix("fm-e2e-").tempdir()?;
        let data = directory.path().join("data");
        std::fs::create_dir(&data)?;
        let assets = directory.path().join("assets");
        std::fs::create_dir(&assets)?;
        let db_url = format!("sqlite://{}/database.sqlite?mode=rwc", data.display());
        let db = firemage_queries::connect(&db_url).await?;
        firemage_queries::bootstrap(&db, "admin".into(), firemage_auth::hash_password(PASSWORD)?)
            .await?;
        let server_port = free_port()?;
        let url = format!("http://127.0.0.1:{server_port}");
        let client_secret = confidential.then(|| "fixture-confidential-secret".to_owned());
        let provider = if with_oidc {
            Some(MockProvider::new(directory.path(), &url, client_secret.clone()).await?)
        } else {
            None
        };
        if let Some(provider) = &provider {
            firemage_queries::add_user_with_issuer(
                &db,
                "federated".into(),
                None,
                false,
                Some("linked-subject".into()),
                Some(provider.issuer.clone()),
            )
            .await?;
        }
        db.close().await?;
        let config = firemage_config::Config {
            server: firemage_config::Server {
                oidc_issuer: provider.as_ref().map(|p| p.issuer.clone()),
                oidc_client_secret: client_secret,
                oidc_client_id: provider.as_ref().map(|_| "browser-client".into()),
                oidc_ca_cert: provider.as_ref().map(|p| p.ca_cert.clone()),
                listen: Some(format!("127.0.0.1:{server_port}")),
                public_url: Some(url.clone()),
                database: Some(db_url),
                data_dir: Some(data),
                firecracker: Some(directory.path().join("intentionally-missing-firecracker")),
                local_asset_roots: Some(vec![assets]),
                session_ttl: Some(600),
                ..Default::default()
            },
            ..Default::default()
        };
        let config_path = directory.path().join("config.toml");
        std::fs::write(&config_path, toml::to_string(&config)?)?;
        let mut command = Command::new(binary);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("FIREMAGE_") {
                command.env_remove(key);
            }
        }
        command
            .args(["--config"])
            .arg(&config_path)
            .arg("serve")
            .current_dir(directory.path());
        let mut processes = Processes(vec![logged(
            &mut command,
            screenshots
                .parent()
                .expect("results parent")
                .join(format!("{name}-server.log")),
        )?]);
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()?;
        ready(&client, &format!("{url}/health"), &mut processes.0[0]).await?;
        let auth: serde_json::Value = client
            .post(format!("{url}/v1/auth/login"))
            .json(&serde_json::json!({"username":"admin","password":PASSWORD}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let token = auth["token"]
            .as_str()
            .context("fixture administrator login")?
            .to_owned();
        let port = free_port()?;
        let driver_url = format!("http://127.0.0.1:{port}");
        let mut command = Command::new("chromedriver");
        command.args([format!("--port={port}"), "--allowed-ips=127.0.0.1".into()]);
        processes.0.push(logged(
            &mut command,
            screenshots
                .parent()
                .expect("results parent")
                .join(format!("{name}-chromedriver.log")),
        )?);
        ready(
            &client,
            &format!("{driver_url}/status"),
            processes.0.last_mut().expect("driver process"),
        )
        .await?;
        let mut caps = DesiredCapabilities::chrome();
        for arg in [
            "--headless=new",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-gpu",
            "--window-size=1440,1000",
            "--no-proxy-server",
        ] {
            caps.add_arg(arg)?;
        }
        caps.accept_insecure_certs(with_oidc)?;
        if let Some(path) = browser_path() {
            caps.set_binary(&path)?;
        }
        let driver = WebDriver::new(&driver_url, caps)
            .await
            .context("start desktop Chromium session")?;
        driver.set_window_rect(0, 0, 1440, 1000).await?;
        driver.goto(&url).await?;
        Ok(Self {
            driver,
            url,
            config_path,
            token,
            client,
            name: name.into(),
            screenshots,
            _processes: processes,
            _provider: provider,
            _directory: directory,
        })
    }
    pub async fn api(&self, path: &str) -> Result<serde_json::Value> {
        Ok(self
            .client
            .get(format!("{}{path}", self.url))
            .bearer_auth(&self.token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    pub async fn guarded(future: impl std::future::Future<Output = Result<()>>) -> Result<()> {
        use futures_util::FutureExt;
        match std::panic::AssertUnwindSafe(future).catch_unwind().await {
            Ok(result) => result,
            Err(panic) => {
                let message = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                anyhow::bail!("browser journey panicked: {message}")
            }
        }
    }
    pub fn oidc_callback(&self) -> Result<String> {
        self._provider
            .as_ref()
            .and_then(|provider| provider.last_callback.lock().ok()?.clone())
            .context("OIDC provider has no completed callback")
    }
    pub async fn complete(self, result: Result<()>) -> Result<()> {
        let suffix = if result.is_ok() { "passed" } else { "failure" };
        let evidence = self.screenshot(suffix).await;
        if result.is_err()
            && let Ok(source) = self.driver.source().await
        {
            let _ = std::fs::write(
                self.screenshots
                    .parent()
                    .expect("results parent")
                    .join(format!("{}-failure.html", self.name)),
                source,
            );
        }
        let quit = self.driver.clone().quit().await;
        result?;
        evidence?;
        quit?;
        Ok(())
    }
}
