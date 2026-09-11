use crate::Runtime;
use anyhow::Context;
use firemage_wire::VmSpec;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

impl Runtime {
    async fn snapshot_network(&self, owner: &str, spec: &VmSpec) -> anyhow::Result<Value> {
        let mut attachment = spec.network.clone();
        let definition = if let Some(attachment) = &mut attachment {
            let row = firemage_queries::network(&self.db, owner, &attachment.network).await?;
            attachment.network = row.id;
            serde_json::from_str::<Value>(&row.spec)?
        } else {
            Value::Null
        };
        Ok(json!({"attachment": attachment, "definition": definition}))
    }
    pub(crate) async fn record_snapshot(
        &self,
        row: &firemage_orm::vms::Model,
        state: &str,
        memory: &str,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode != firemage_wire::IsolationMode::Jailed
            && !self.is_restricted_network(&row.owner_id, &spec).await?
        {
            return Ok(());
        }
        let record = self.snapshot_record(row, state, memory).await?;
        let path = self.snapshot_record_path(&row.id, &record)?;
        firemage_config::write_private(&path, &serde_json::to_vec(&record)?)
    }
    pub(crate) async fn ensure_snapshot(
        &self,
        row: &firemage_orm::vms::Model,
        state: &str,
        memory: &str,
    ) -> anyhow::Result<()> {
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        if spec.security.mode != firemage_wire::IsolationMode::Jailed
            && !self.is_restricted_network(&row.owner_id, &spec).await?
        {
            return Ok(());
        }
        let mut expected = self.snapshot_record(row, state, memory).await?;
        let path = self.snapshot_record_path(&row.id, &expected)?;
        let mut saved: Value = serde_json::from_slice(&tokio::fs::read(path).await.context(
            "Firemage-only restore requires a snapshot created by this VM through Firemage",
        )?)?;
        self.normalize_snapshot_network(&row.owner_id, &mut saved, &mut expected)
            .await?;
        anyhow::ensure!(
            saved == expected,
            "snapshot provenance or network configuration changed; restore requires this VM's unchanged managed snapshot"
        );
        Ok(())
    }
    async fn snapshot_record(
        &self,
        row: &firemage_orm::vms::Model,
        state: &str,
        memory: &str,
    ) -> anyhow::Result<Value> {
        let state = tokio::fs::canonicalize(state).await?;
        let memory = tokio::fs::canonicalize(memory).await?;
        anyhow::ensure!(
            tokio::fs::metadata(&memory).await?.is_file(),
            "snapshot memory must be a regular file"
        );
        let spec: VmSpec = serde_json::from_str(&row.spec)?;
        let mut record = json!({"version": 2, "vm_id": row.id, "state": state, "memory": memory, "state_sha256": state_hash(&state).await?, "network": self.snapshot_network(&row.owner_id, &spec).await?, "security": spec.security});
        if let Some(terminal) = spec.web_terminal {
            record["web_terminal"] = serde_json::to_value(terminal)?;
        }
        Ok(record)
    }
    fn snapshot_record_path(&self, id: &str, record: &Value) -> anyhow::Result<PathBuf> {
        let state = record["state"]
            .as_str()
            .context("invalid snapshot state path")?;
        Ok(self
            .directory(id)
            .join("snapshots")
            .join(format!("{:x}.json", Sha256::digest(state.as_bytes()))))
    }
}
impl Runtime {
    async fn normalize_snapshot_network(
        &self,
        owner: &str,
        saved: &mut Value,
        expected: &mut Value,
    ) -> anyhow::Result<()> {
        if saved.get("version").is_none() {
            expected
                .as_object_mut()
                .context("invalid expected snapshot record")?
                .remove("version");
            if let Some(attachment) = saved
                .get("network")
                .and_then(|network| network.get("attachment"))
                && !attachment.is_null()
            {
                let name = attachment
                    .get("network")
                    .and_then(Value::as_str)
                    .context("invalid legacy snapshot network reference")?;
                anyhow::ensure!(
                    saved["network"]["definition"]["name"].as_str() == Some(name),
                    "legacy snapshot network name differs from its definition"
                );
                let network = firemage_queries::network_by_name(&self.db, owner, name).await
                    .context("legacy private snapshot network was renamed or removed; restore requires its original network name")?;
                anyhow::ensure!(
                    expected["network"]["attachment"]["network"] == network.id,
                    "legacy snapshot belongs to another network"
                );
                saved
                    .get_mut("network")
                    .and_then(|network| network.get_mut("attachment"))
                    .and_then(Value::as_object_mut)
                    .context("invalid legacy snapshot attachment")?
                    .insert("network".into(), Value::String(network.id));
            }
        }
        for record in [saved, expected] {
            if let Some(definition) = record
                .get_mut("network")
                .and_then(|network| network.get_mut("definition"))
                .and_then(Value::as_object_mut)
            {
                definition.remove("name");
            }
        }
        Ok(())
    }
}

async fn state_hash(path: &Path) -> anyhow::Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    anyhow::ensure!(
        metadata.is_file() && metadata.len() <= 64 * 1024 * 1024,
        "snapshot state must be a regular file up to 64 MiB"
    );
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0;
    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        total += count;
        anyhow::ensure!(total <= 64 * 1024 * 1024, "snapshot state exceeds 64 MiB");
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
