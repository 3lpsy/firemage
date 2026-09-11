use crate::Runtime;
use firemage_wire::{BootFile, EnvironmentValue, FileEncoding, VmSpec};

impl Runtime {
    pub(crate) async fn seed_files(
        &self,
        owner: &str,
        spec: &VmSpec,
    ) -> anyhow::Result<Vec<BootFile>> {
        let mut files = spec.files.clone();
        if spec.web_terminal.is_some() {
            files.push(self.guest_binary_file().await?);
        }
        files.extend(self.attachment_files(owner, spec).await?);
        files.extend(self.secret_attachment_files(owner, spec).await?);
        if let Some(workload) = &spec.workload {
            files.push(file(
                "firemage/workload.sh",
                crate::workload::script(workload),
            ));
        }
        let mut values = std::collections::BTreeMap::new();
        for (name, value) in &spec.environment {
            values.insert(
                name.clone(),
                match value {
                    EnvironmentValue::Plain(value) => value.clone(),
                    EnvironmentValue::Secret { secret } => {
                        self.secrets().await?.resolve(owner, secret).await?
                    }
                },
            );
        }
        if let Some(net) = &spec.network {
            let row = firemage_queries::network(&self.db, owner, &net.network).await?;
            let network: firemage_wire::NetworkSpec = serde_json::from_str(&row.spec)?;
            files.push(file("firemage/network.sh", format!("ip link set eth0 up\nip addr flush dev eth0\nip addr add {}/{} dev eth0\nip route replace default via {} dev eth0\n", net.address, network.subnet.prefix_len(), network.gateway)));
            if matches!(network.policy, firemage_wire::NetworkPolicy::FiremageOnly) {
                let port = spec
                    .egress
                    .as_ref()
                    .and_then(|e| e.http.as_ref())
                    .map(|http| http.port)
                    .unwrap_or(spec.egress_http_port);
                let proxy = format!("http://{}:{}", network.gateway, port);
                for name in ["http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"] {
                    values.insert(name.into(), proxy.clone());
                }
                for name in ["no_proxy", "NO_PROXY"] {
                    values.insert(name.into(), String::new());
                }
                for name in [
                    "SSL_CERT_FILE",
                    "REQUESTS_CA_BUNDLE",
                    "NODE_EXTRA_CA_CERTS",
                    "CURL_CA_BUNDLE",
                ] {
                    values.insert(name.into(), "/firemage/input/firemage/ca.pem".into());
                }
                files.push(file(
                    "firemage/ca.pem",
                    self.egress().await?.ca_certificate_pem(),
                ));
            }
        }
        if !values.is_empty() {
            let script = values
                .into_iter()
                .map(|(name, value)| {
                    format!("export {name}='{}'\n", value.replace('\'', "'\"'\"'"))
                })
                .collect::<String>();
            files.push(file("firemage/environment.sh", script));
        }
        Ok(files)
    }
}
fn file(path: &str, content: String) -> BootFile {
    BootFile {
        path: path.into(),
        content,
        encoding: FileEncoding::Utf8,
        destination: None,
        uid: 0,
        gid: 0,
        mode: 0o600,
    }
}

#[cfg(test)]
#[path = "seed_tests.rs"]
mod tests;
