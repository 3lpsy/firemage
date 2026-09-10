use firemage_egress_policy::{EgressPolicy, TcpTunnel, UpstreamProxy, ValueSource};
use firemage_egress_proxy::EgressManager;
use firemage_request_signing::SecretResolver;
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpSocket, TcpStream},
};
struct Secrets;
#[async_trait::async_trait]
impl SecretResolver for Secrets {
    async fn resolve(&self, name: &str) -> anyhow::Result<String> {
        assert_eq!(name, "proxy-password");
        Ok("password".into())
    }
}
fn port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
async fn echo() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (mut read, mut write) = stream.split();
        let _ = tokio::io::copy(&mut read, &mut write).await;
    });
    port
}

#[tokio::test]
async fn authenticated_http_and_socks5_chain_fixed_ip_tunnels() {
    for scheme in ["http", "socks5"] {
        let target = echo().await;
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = upstream.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut stream, _) = upstream.accept().await.unwrap();
            if scheme == "http" {
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    headers.push(stream.read_u8().await.unwrap());
                }
                let headers = String::from_utf8(headers).unwrap();
                assert!(headers.starts_with(&format!("CONNECT 127.0.0.1:{target} HTTP/1.1\r\n")));
                assert!(headers.contains("Proxy-Authorization: Basic dXNlcjpwYXNzd29yZA==\r\n"));
                stream
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .await
                    .unwrap();
            } else {
                let mut greeting = [0; 3];
                stream.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, [5, 1, 2]);
                stream.write_all(&[5, 2]).await.unwrap();
                assert_eq!(stream.read_u8().await.unwrap(), 1);
                let length = stream.read_u8().await.unwrap();
                let mut username = vec![0; length as usize];
                stream.read_exact(&mut username).await.unwrap();
                assert_eq!(username, b"user");
                let length = stream.read_u8().await.unwrap();
                let mut password = vec![0; length as usize];
                stream.read_exact(&mut password).await.unwrap();
                assert_eq!(password, b"password");
                stream.write_all(&[1, 0]).await.unwrap();
                let mut request = [0; 10];
                stream.read_exact(&mut request).await.unwrap();
                assert_eq!(&request[..8], &[5, 1, 0, 1, 127, 0, 0, 1]);
                assert_eq!(u16::from_be_bytes([request[8], request[9]]), target);
                stream
                    .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                    .await
                    .unwrap();
            }
            let mut destination = TcpStream::connect((Ipv4Addr::LOCALHOST, target))
                .await
                .unwrap();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut destination).await;
        });
        let directory = tempfile::tempdir().unwrap();
        let manager = EgressManager::new(directory.path()).await.unwrap();
        let proxy = port();
        let policy = EgressPolicy {
            upstream: Some(UpstreamProxy {
                url: format!("{scheme}://127.0.0.1:{upstream_port}"),
                username: Some(ValueSource::Literal("user".into())),
                password: Some(ValueSource::Secret {
                    secret: "proxy-password".into(),
                    prefix: String::new(),
                }),
                ca_pem: None,
            }),
            tunnels: vec![TcpTunnel {
                name: "binary".into(),
                listen_port: proxy,
                target_host: "localhost".into(),
                target_port: target,
                allowed_ips: vec!["127.0.0.0/8".parse().unwrap()],
            }],
            ..Default::default()
        };
        manager
            .register(
                "vm",
                Ipv4Addr::new(127, 0, 0, 2),
                Ipv4Addr::LOCALHOST,
                policy,
                Arc::new(Secrets),
            )
            .await
            .unwrap();
        let socket = TcpSocket::new_v4().unwrap();
        socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
        let mut stream = socket
            .connect(SocketAddr::from((Ipv4Addr::LOCALHOST, proxy)))
            .await
            .unwrap();
        stream.write_all(&[0, 255, 12, 34]).await.unwrap();
        stream.shutdown().await.unwrap();
        let mut result = [0; 4];
        stream.read_exact(&mut result).await.unwrap();
        assert_eq!(result, [0, 255, 12, 34]);
        drop(stream);
        manager.unregister("vm").await;
        task.await.unwrap();
    }
}

#[tokio::test]
async fn upstream_failure_never_falls_back_to_direct() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let dead_proxy = port();
    let listen = port();
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let policy = EgressPolicy {
        upstream: Some(UpstreamProxy {
            url: format!("http://127.0.0.1:{dead_proxy}"),
            username: None,
            password: None,
            ca_pem: None,
        }),
        tunnels: vec![TcpTunnel {
            name: "fixed".into(),
            listen_port: listen,
            target_host: "127.0.0.1".into(),
            target_port,
            allowed_ips: vec![],
        }],
        ..Default::default()
    };
    manager
        .register(
            "vm",
            Ipv4Addr::new(127, 0, 0, 2),
            Ipv4Addr::LOCALHOST,
            policy,
            Arc::new(Secrets),
        )
        .await
        .unwrap();
    let socket = TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
    let mut stream = socket
        .connect(SocketAddr::from((Ipv4Addr::LOCALHOST, listen)))
        .await
        .unwrap();
    assert!(stream.read_u8().await.is_err());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), target.accept())
            .await
            .is_err()
    );
}
