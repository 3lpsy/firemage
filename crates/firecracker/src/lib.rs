use anyhow::Context;
use serde_json::Value;
use std::{path::Path, time::Duration};

#[derive(Clone)]
pub struct Firecracker {
    client: reqwest::Client,
}
impl Firecracker {
    pub fn new(socket: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .unix_socket(socket.to_path_buf())
                .timeout(Duration::from_secs(120))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> anyhow::Result<(u16, Value)> {
        anyhow::ensure!(
            matches!(method, "GET" | "PUT" | "PATCH"),
            "Firecracker method must be GET, PUT or PATCH"
        );
        anyhow::ensure!(
            path.starts_with('/')
                && !path.starts_with("//")
                && !path.contains(['?', '#', '\\'])
                && !path.split('/').any(|p| p == ".." || p == ".")
                && path
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/-_".contains(&b)),
            "invalid Firecracker API path"
        );
        let mut request = self
            .client
            .request(method.parse()?, format!("http://localhost{path}"));
        if method == "GET" {
            request = request.timeout(Duration::from_secs(3));
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .context("Firecracker socket request failed")?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        Ok((status, value))
    }
    pub async fn call(&self, method: &str, path: &str, body: Value) -> anyhow::Result<Value> {
        let (status, value) = self
            .request(
                method,
                path,
                if body.is_null() { None } else { Some(&body) },
            )
            .await?;
        anyhow::ensure!(
            (200..300).contains(&status),
            "Firecracker HTTP {status}: {value}"
        );
        Ok(value)
    }
    pub async fn state(&self) -> anyhow::Result<String> {
        self.call("GET", "/", Value::Null).await?["state"]
            .as_str()
            .map(str::to_owned)
            .context("Firecracker did not return instance state")
    }
}
