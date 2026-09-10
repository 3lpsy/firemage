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
        let mut request = self.authenticated_request(method, path)?;
        if let Some(body) = body {
            request = request.json(body);
        }
        response_json(request).await
    }
    pub async fn put_bytes<T: DeserializeOwned>(
        &self,
        path: &str,
        bytes: Vec<u8>,
    ) -> anyhow::Result<T> {
        self.request_bytes("PUT", path, bytes).await
    }
    pub async fn post_bytes<T: DeserializeOwned>(
        &self,
        path: &str,
        bytes: Vec<u8>,
    ) -> anyhow::Result<T> {
        self.request_bytes("POST", path, bytes).await
    }
    pub async fn get_bytes(&self, path: &str, maximum: u64) -> anyhow::Result<Vec<u8>> {
        let mut response = self
            .authenticated_request("GET", path)?
            .send()
            .await
            .context("connecting to Firemage")?;
        let status = response.status();
        anyhow::ensure!(status.is_success(), "Firemage HTTP {status}");
        anyhow::ensure!(
            response.content_length().is_none_or(|size| size <= maximum),
            "Firemage download exceeds {maximum} bytes"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            anyhow::ensure!(
                (bytes.len() as u64).saturating_add(chunk.len() as u64) <= maximum,
                "Firemage download exceeds {maximum} bytes"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    async fn request_bytes<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        bytes: Vec<u8>,
    ) -> anyhow::Result<T> {
        response_json(
            self.authenticated_request(method, path)?
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(bytes),
        )
        .await
    }
    fn authenticated_request(
        &self,
        method: &str,
        path: &str,
    ) -> anyhow::Result<reqwest::RequestBuilder> {
        let mut request = self
            .http
            .request(method.parse()?, format!("{}{path}", self.base));
        if let Some(token) = &self.token {
            request = request.bearer_auth(token);
        }
        Ok(request)
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

async fn response_json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> anyhow::Result<T> {
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
