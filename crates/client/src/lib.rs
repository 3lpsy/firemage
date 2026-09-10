use anyhow::Context;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{path::Path, time::Duration};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}
impl Client {
    pub fn new(base: &str, socket: Option<&Path>, token: Option<String>) -> anyhow::Result<Self> {
        Self::with_ca(base, socket, token, None)
    }
    pub fn with_ca(
        base: &str,
        socket: Option<&Path>,
        token: Option<String>,
        ca: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let url = reqwest::Url::parse(base)?;
        anyhow::ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "invalid server URL"
        );
        let loopback = url.host_str().is_some_and(|h| {
            h == "localhost"
                || h.parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        anyhow::ensure!(
            url.scheme() == "https" || (url.scheme() == "http" && (socket.is_some() || loopback)),
            "remote server URL must use HTTPS"
        );
        let mut http = reqwest::Client::builder()
            .timeout(Duration::from_secs(900))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(ca) = ca {
            http = http.add_root_certificate(reqwest::Certificate::from_pem(&std::fs::read(ca)?)?);
        }
        if let Some(socket) = socket {
            http = http.unix_socket(socket.to_path_buf());
        }
        Ok(Self {
            http: http.build()?,
            base: base.trim_end_matches('/').into(),
            token,
        })
    }
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> anyhow::Result<T> {
        let mut request = self
            .http
            .request(method.parse()?, format!("{}{path}", self.base));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.context("connecting to Firemage")?;
        let status = response.status();
        let text = response.text().await?;
        anyhow::ensure!(status.is_success(), "Firemage HTTP {status}: {text}");
        Ok(serde_json::from_str(if text.is_empty() {
            "null"
        } else {
            &text
        })?)
    }
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.request("GET", path, None::<&Value>).await
    }
    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<T> {
        self.request("POST", path, Some(body)).await
    }
    pub async fn delete(&self, path: &str) -> anyhow::Result<Value> {
        self.request("DELETE", path, None::<&Value>).await
    }
}
