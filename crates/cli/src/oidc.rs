use anyhow::Context;
use serde_json::{Value, json};

pub async fn login(client: &firemage_client::Client) -> anyhow::Result<firemage_wire::Token> {
    let config: Value = client.get("/v1/auth/oidc").await?;
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let client_id = config["client_id"]
        .as_str()
        .context("missing OIDC client id")?;
    let device_url = config["device_authorization_endpoint"]
        .as_str()
        .context("provider does not support device authorization")?;
    let token_url = config["token_endpoint"]
        .as_str()
        .context("missing OIDC token endpoint")?;
    anyhow::ensure!(
        reqwest::Url::parse(device_url)?.scheme() == "https"
            && reqwest::Url::parse(token_url)?.scheme() == "https",
        "OIDC endpoints must use HTTPS"
    );
    let device: Value = http
        .post(device_url)
        .form(&[("client_id", client_id), ("scope", "openid profile")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    eprintln!(
        "Open {} and enter {}",
        device["verification_uri"]
            .as_str()
            .context("missing verification URL")?,
        device["user_code"].as_str().context("missing user code")?
    );
    let code = device["device_code"]
        .as_str()
        .context("missing device code")?;
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(device["expires_in"].as_u64().unwrap_or(600).min(1800));
    let mut interval = device["interval"].as_u64().unwrap_or(5).clamp(1, 60);
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        let response: Value = http
            .post(token_url)
            .form(&[
                ("client_id", client_id),
                ("device_code", code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json()
            .await?;
        match response["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval = (interval + 5).min(60);
                continue;
            }
            Some(error) => anyhow::bail!("OIDC authorization failed: {error}"),
            None => {
                let id_token = response["id_token"]
                    .as_str()
                    .context("provider did not issue an OIDC identity token")?;
                return client
                    .post("/v1/auth/oidc", &json!({"id_token":id_token}))
                    .await;
            }
        }
    }
    anyhow::bail!("OIDC authorization timed out")
}
