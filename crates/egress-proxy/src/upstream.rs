use crate::transport::Stream;
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) async fn http_connect(
    stream: &mut Stream,
    target: SocketAddr,
    credentials: Option<(String, String)>,
) -> Result<()> {
    let mut request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n");
    if let Some((username, password)) = credentials {
        ensure!(
            !username.contains(':'),
            "HTTP proxy username cannot contain colon"
        );
        request.push_str(&format!(
            "Proxy-Authorization: Basic {}\r\n",
            STANDARD.encode(format!("{username}:{password}"))
        ));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await?;
    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        ensure!(
            response.len() < 16384,
            "upstream proxy response headers too large"
        );
        response.push(stream.read_u8().await?);
    }
    let response = std::str::from_utf8(&response)?;
    let status = response
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>();
    ensure!(
        status.len() >= 2 && matches!(status[0], "HTTP/1.1" | "HTTP/1.0") && status[1] == "200",
        "upstream proxy denied CONNECT"
    );
    Ok(())
}

pub(crate) async fn socks5(
    stream: &mut Stream,
    target: SocketAddr,
    credentials: Option<(String, String)>,
) -> Result<()> {
    let method = if credentials.is_some() { 2 } else { 0 };
    stream.write_all(&[5, 1, method]).await?;
    let mut selected = [0; 2];
    stream.read_exact(&mut selected).await?;
    ensure!(
        selected == [5, method],
        "SOCKS5 authentication method rejected"
    );
    if let Some((username, password)) = credentials {
        ensure!(
            !username.is_empty()
                && username.len() <= 255
                && !password.is_empty()
                && password.len() <= 255,
            "SOCKS5 credentials must be 1-255 bytes"
        );
        let mut auth = vec![1, username.len() as u8];
        auth.extend(username.as_bytes());
        auth.push(password.len() as u8);
        auth.extend(password.as_bytes());
        stream.write_all(&auth).await?;
        stream.read_exact(&mut selected).await?;
        ensure!(selected == [1, 0], "SOCKS5 authentication failed");
    }
    let mut request = vec![5, 1, 0];
    match target.ip() {
        IpAddr::V4(ip) => {
            request.push(1);
            request.extend(ip.octets());
        }
        IpAddr::V6(ip) => {
            request.push(4);
            request.extend(ip.octets());
        }
    }
    request.extend(target.port().to_be_bytes());
    stream.write_all(&request).await?;
    let mut reply = [0; 4];
    stream.read_exact(&mut reply).await?;
    ensure!(
        reply[0] == 5 && reply[1] == 0 && reply[2] == 0,
        "SOCKS5 connection rejected"
    );
    let length = match reply[3] {
        1 => 4,
        4 => 16,
        3 => stream.read_u8().await? as usize,
        _ => anyhow::bail!("invalid SOCKS5 reply"),
    };
    let mut rest = vec![0; length + 2];
    stream.read_exact(&mut rest).await?;
    Ok(())
}
