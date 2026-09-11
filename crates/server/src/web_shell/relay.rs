use crate::App;
use anyhow::Context;
use axum::extract::ws::{Message, WebSocket};
use firemage_guest_protocol::{ClientMessage, MAX_FRAME_BYTES, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) async fn run(
    mut socket: WebSocket,
    app: App,
    row: firemage_orm::vms::Model,
    credential: firemage_orm::credentials::Model,
    _permit: tokio::sync::OwnedSemaphorePermit,
) {
    if let Err(error) = relay(&mut socket, &app, &row, &credential).await {
        let _ = send(
            &mut socket,
            &ServerMessage::Error {
                message: error.to_string(),
            },
        )
        .await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(2), socket.close()).await;
}

async fn relay(
    socket: &mut WebSocket,
    app: &App,
    row: &firemage_orm::vms::Model,
    credential: &firemage_orm::credentials::Model,
) -> anyhow::Result<()> {
    let (stream, command) = app
        .runtime
        .connect_web_shell(&row.owner_id, &row.id)
        .await?;
    let (read, mut write) = stream.into_split();
    write_message(
        &mut write,
        &ClientMessage::Open {
            version: firemage_guest_protocol::VERSION,
            command,
            rows: 24,
            cols: 80,
        },
    )
    .await?;
    // The stream retains an in-progress frame across input and heartbeat branches.
    let mut incoming = Box::pin(futures_util::stream::try_unfold(
        read,
        |mut read| async move {
            let length = read.read_u32().await? as usize;
            anyhow::ensure!(
                (1..=MAX_FRAME_BYTES).contains(&length),
                "invalid guest shell frame size"
            );
            let mut bytes = vec![0; length];
            tokio::time::timeout(Duration::from_secs(5), read.read_exact(&mut bytes)).await??;
            let message: ServerMessage = serde_json::from_slice(&bytes)?;
            anyhow::Ok(Some((message, read)))
        },
    ));
    let opened = std::time::Instant::now();
    let mut ready = false;
    let mut last_browser_frame = opened;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
    let mut keepalive = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            frame = incoming.next() => {
                let Some(frame) = frame else { break; };
                let frame = frame?;
                match &frame {
                    ServerMessage::Ready => { anyhow::ensure!(!ready, "guest sent duplicate readiness"); ready = true; },
                    ServerMessage::Error { .. } => (),
                    _ => anyhow::ensure!(ready, "guest shell did not send readiness"),
                }
                if let ServerMessage::Output { data } = &frame { firemage_guest_protocol::decode_data(data)?; }
                send(socket, &frame).await?;
                if matches!(frame, ServerMessage::Exit { .. } | ServerMessage::Error { .. }) { break; }
            }
            frame = socket.recv() => {
                let Some(frame) = frame else { break; };
                last_browser_frame = std::time::Instant::now();
                match frame? {
                    Message::Text(text) => {
                        let message: ClientMessage = serde_json::from_str(&text)?;
                        validate_input(&message)?;
                        write_message(&mut write, &message).await?;
                        if matches!(message, ClientMessage::Close) { break; }
                    }
                    Message::Close(_) => break,
                    Message::Ping(_) | Message::Pong(_) => {},
                    Message::Binary(_) => anyhow::bail!("Web Shell expects JSON text frames"),
                }
            }
            _ = keepalive.tick() => {
                tokio::time::timeout(Duration::from_secs(5), socket.send(Message::Ping(Vec::new().into()))).await??;
            }
            _ = heartbeat.tick() => {
                anyhow::ensure!(ready || opened.elapsed() < Duration::from_secs(10), "guest shell did not become ready");
                anyhow::ensure!(last_browser_frame.elapsed() < Duration::from_secs(90), "browser shell connection stopped responding");
                let (user, current) = firemage_queries::authenticate(&app.runtime.db, &credential.token_hash).await?.context("shell session authorization expired")?;
                anyhow::ensure!(user.admin && current.id == credential.id, "shell session authorization expired");
                let current = firemage_queries::vm(&app.runtime.db, &row.owner_id, &row.id).await?;
                anyhow::ensure!(current.pid == row.pid && current.process_start == row.process_start,
                    "VM process changed; reconnect Web Shell");
                app.runtime.ensure_web_shell(&row.owner_id, &row.id).await?;
            }
        }
    }
    // Dropping the vsock stream also causes guest cleanup if the close frame cannot be sent.
    let _ = write_message(&mut write, &ClientMessage::Close).await;
    Ok(())
}

pub(super) fn validate_input(message: &ClientMessage) -> anyhow::Result<()> {
    match message {
        ClientMessage::Open { .. } => {
            anyhow::bail!("shell command is managed by the VM configuration")
        }
        ClientMessage::Input { data } => {
            firemage_guest_protocol::decode_data(data)?;
        }
        ClientMessage::Resize { rows, cols } => {
            firemage_guest_protocol::validate_size(*rows, *cols)?
        }
        ClientMessage::Close => (),
    }
    Ok(())
}
async fn write_message(
    write: &mut tokio::net::unix::OwnedWriteHalf,
    message: &ClientMessage,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(message)?;
    anyhow::ensure!(bytes.len() <= MAX_FRAME_BYTES, "shell frame exceeds limit");
    tokio::time::timeout(Duration::from_secs(5), async {
        write.write_u32(bytes.len() as u32).await?;
        write.write_all(&bytes).await
    })
    .await??;
    Ok(())
}
async fn send(socket: &mut WebSocket, message: &ServerMessage) -> anyhow::Result<()> {
    tokio::time::timeout(
        Duration::from_secs(5),
        socket.send(Message::Text(serde_json::to_string(message)?.into())),
    )
    .await??;
    Ok(())
}
