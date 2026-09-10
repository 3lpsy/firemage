use crate::args::Vm;
use firemage_client::Client;
use firemage_wire::{VmAction, VmSpec, VmState};
use serde_json::Value;

async fn action(client: &Client, id: &str, action: VmAction) -> anyhow::Result<Value> {
    client.post(&format!("/v1/vms/{id}/actions"), &action).await
}
async fn wait(client: &Client, id: &str, timeout: u64) -> anyhow::Result<Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
    loop {
        let vm: firemage_wire::Vm = client
            .post(&format!("/v1/vms/{id}/actions"), &VmAction::Refresh)
            .await?;
        if vm.state == VmState::Stopped {
            return Ok(serde_json::to_value(vm)?);
        }
        anyhow::ensure!(
            vm.state != VmState::Failed,
            "VM failed: {}",
            vm.error.unwrap_or_default()
        );
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "timed out waiting for VM {id}; VM remains registered"
        );
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
pub async fn run(command: Vm, client: &Client) -> anyhow::Result<()> {
    let value: Value = match command {
        Vm::List => client.get("/v1/vms").await?,
        Vm::Create { file, input } => client.post("/v1/vms", &spec(&file, input)?).await?,
        Vm::Run {
            file,
            input,
            wait: should_wait,
            timeout,
        } => {
            let vm: firemage_wire::Vm = client.post("/v1/vms", &spec(&file, input)?).await?;
            eprintln!("VM {}", vm.id);
            let started = action(client, &vm.id, VmAction::Start).await?;
            if should_wait {
                wait(client, &vm.id, timeout).await?
            } else {
                started
            }
        }
        Vm::Update { id, file, input } => {
            client
                .request("PUT", &format!("/v1/vms/{id}"), Some(&spec(&file, input)?))
                .await?
        }
        Vm::Egress { id } => client.get(&format!("/v1/vms/{id}/egress")).await?,
        Vm::Get { id } => client.get(&format!("/v1/vms/{id}")).await?,
        Vm::Delete { id } => client.delete(&format!("/v1/vms/{id}")).await?,
        Vm::Launch { id } => action(client, &id, VmAction::Launch).await?,
        Vm::Prepare { id } => action(client, &id, VmAction::Prepare).await?,
        Vm::Start { id } => action(client, &id, VmAction::Start).await?,
        Vm::Pause { id } => action(client, &id, VmAction::Pause).await?,
        Vm::Resume { id } => action(client, &id, VmAction::Resume).await?,
        Vm::Shutdown { id } => action(client, &id, VmAction::Shutdown).await?,
        Vm::Stop { id } => action(client, &id, VmAction::Stop).await?,
        Vm::Refresh { id } => action(client, &id, VmAction::Refresh).await?,
        Vm::Wait { id, timeout } => wait(client, &id, timeout).await?,
        Vm::Snapshot {
            id,
            snapshot_path,
            memory_path,
        } => {
            action(
                client,
                &id,
                VmAction::Snapshot {
                    snapshot_path,
                    memory_path,
                },
            )
            .await?
        }
        Vm::Restore {
            id,
            snapshot_path,
            memory_path,
        } => {
            action(
                client,
                &id,
                VmAction::Restore {
                    snapshot_path,
                    memory_path,
                },
            )
            .await?
        }
        Vm::Metadata { id, file } => {
            action(
                client,
                &id,
                VmAction::Metadata {
                    value: crate::toml_file(&file)?,
                },
            )
            .await?
        }
        Vm::Api {
            id,
            method,
            path,
            file,
        } => {
            let response: firemage_wire::RawResponse = client
                .post(
                    &format!("/v1/vms/{id}/firecracker"),
                    &firemage_wire::RawRequest {
                        method,
                        path,
                        body: file.as_deref().map(crate::toml_file).transpose()?,
                    },
                )
                .await?;
            anyhow::ensure!(
                (200..300).contains(&response.status),
                "Firecracker HTTP {}: {}",
                response.status,
                response.body
            );
            response.body
        }
        Vm::Cp {
            id,
            guest_path,
            output,
        } => {
            use base64::Engine;
            firemage_wire::ensure_guest_path(&guest_path)?;
            let value: Value = client
                .get(&format!("/v1/vms/{id}/files?path={guest_path}"))
                .await?;
            let bytes = base64::engine::general_purpose::STANDARD.decode(
                value["base64"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid file response"))?,
            )?;
            tokio::fs::write(output, bytes).await?;
            Value::Null
        }
        Vm::Logs { id } => client.get(&format!("/v1/vms/{id}/logs")).await?,
    };
    crate::print(&value)
}

fn spec(file: &std::path::Path, inputs: Vec<String>) -> anyhow::Result<VmSpec> {
    use base64::Engine;
    let mut spec: VmSpec = crate::toml_file(file)?;
    for input in inputs {
        let (host, guest) = input
            .rsplit_once(':')
            .ok_or_else(|| anyhow::anyhow!("input must be HOST_PATH:GUEST_PATH"))?;
        let (path, destination) = if let Some(relative) = guest.strip_prefix('/') {
            firemage_wire::ensure_guest_path(relative)?;
            (
                format!("uploads/{}", spec.files.len()),
                Some(guest.to_owned()),
            )
        } else {
            firemage_wire::ensure_guest_path(guest)?;
            (guest.to_owned(), None)
        };
        anyhow::ensure!(
            std::fs::metadata(host)?.len() <= 4 * 1024 * 1024,
            "boot file exceeds 4 MiB"
        );
        spec.files.push(firemage_wire::BootFile {
            path,
            content: base64::engine::general_purpose::STANDARD.encode(std::fs::read(host)?),
            encoding: firemage_wire::FileEncoding::Base64,
            destination,
            uid: 0,
            gid: 0,
            mode: 0o644,
        });
    }
    spec.validate()?;
    Ok(spec)
}
