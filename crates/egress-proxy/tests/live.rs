use firemage_egress_policy::{EgressPolicy, HttpProxy, HttpRule, HttpScheme, TcpTunnel};
use firemage_egress_proxy::{EgressManager, Replacement};
use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpSocket},
};

struct Secrets;
#[async_trait::async_trait]
impl firemage_request_signing::SecretResolver for Secrets {
    async fn resolve(&self, _: &str) -> anyhow::Result<String> {
        anyhow::bail!("no secrets")
    }
}
async fn port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn update(id: &str, guest: u8, policy: EgressPolicy) -> Replacement {
    Replacement {
        id: id.into(),
        guest: Ipv4Addr::new(127, 0, 0, guest),
        gateway: Ipv4Addr::LOCALHOST,
        policy: Some(policy),
        secrets: Arc::new(Secrets),
    }
}
fn client(port: u16, guest: u8) -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).unwrap())
        .local_address(IpAddr::V4(Ipv4Addr::new(127, 0, 0, guest)))
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}
async fn origin() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                loop {
                    let mut headers = Vec::new();
                    while !headers.ends_with(b"\r\n\r\n") {
                        let Ok(byte) = stream.read_u8().await else {
                            return;
                        };
                        headers.push(byte);
                    }
                    if stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    port
}
fn http(port: u16, origin: u16) -> EgressPolicy {
    EgressPolicy {
        inherit_upstream: false,
        upstream: None,
        http: Some(HttpProxy {
            port,
            upstream_ca_pem: None,
            rules: vec![HttpRule {
                host: "127.0.0.1".into(),
                port: origin,
                scheme: HttpScheme::Http,
                methods: vec!["GET".into()],
                path_prefix: "/".into(),
                allowed_ips: vec!["127.0.0.0/8".parse().unwrap()],
                headers: Default::default(),
                signing: None,
            }],
        }),
        tunnels: vec![],
    }
}

#[tokio::test]
async fn replacement_revokes_keepalive_requests_without_affecting_other_vms() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let origin = origin().await;
    let listen = port().await;
    let allowed = http(listen, origin);
    manager
        .replace_many(vec![
            update("first", 2, allowed.clone()),
            update("second", 3, allowed.clone()),
        ])
        .await
        .unwrap();
    let first = client(listen, 2);
    let second = client(listen, 3);
    let url = format!("http://127.0.0.1:{origin}/");
    assert_eq!(
        first.get(&url).send().await.unwrap().text().await.unwrap(),
        "ok"
    );
    let mut denied = allowed;
    denied.http.as_mut().unwrap().rules.clear();
    manager
        .replace_many(vec![update("first", 2, denied)])
        .await
        .unwrap();
    let response = first.get(&url).send().await.unwrap();
    assert!(!response.status().is_success());
    assert_eq!(
        second.get(&url).send().await.unwrap().text().await.unwrap(),
        "ok"
    );
}

#[tokio::test]
async fn replacement_closes_active_tunnels_before_returning() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = listener.local_addr().unwrap().port();
    let listen = port().await;
    let echo = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        while let Ok(byte) = stream.read_u8().await {
            if stream.write_u8(byte).await.is_err() {
                break;
            }
        }
    });
    let allowed = EgressPolicy {
        tunnels: vec![TcpTunnel {
            name: "echo".into(),
            listen_port: listen,
            target_host: "127.0.0.1".into(),
            target_port: origin,
            allowed_ips: vec!["127.0.0.0/8".parse().unwrap()],
        }],
        ..Default::default()
    };
    manager
        .replace_many(vec![update("first", 2, allowed)])
        .await
        .unwrap();
    let socket = TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
    let mut stream = socket
        .connect(([127, 0, 0, 1], listen).into())
        .await
        .unwrap();
    stream.write_u8(42).await.unwrap();
    assert_eq!(stream.read_u8().await.unwrap(), 42);
    manager
        .replace_many(vec![update("first", 2, EgressPolicy::default())])
        .await
        .unwrap();
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    tokio::time::timeout(Duration::from_secs(2), echo)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn listener_conflict_keeps_the_entire_previous_batch_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let origin = origin().await;
    let listen = port().await;
    let allowed = http(listen, origin);
    manager
        .replace_many(vec![update("first", 2, allowed.clone())])
        .await
        .unwrap();
    let occupied = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut conflict = allowed.clone();
    conflict.http.as_mut().unwrap().port = occupied.local_addr().unwrap().port();
    assert!(
        manager
            .replace_many(vec![
                update("first", 2, conflict),
                update("second", 3, allowed)
            ])
            .await
            .is_err()
    );
    assert!(!manager.is_registered("second").await);
    assert_eq!(
        client(listen, 2)
            .get(format!("http://127.0.0.1:{origin}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ok"
    );
}

#[tokio::test]
async fn replacement_drains_connect_upgrade_before_returning() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let listen = port().await;
    let mut allowed = http(listen, 443);
    allowed.http.as_mut().unwrap().rules[0].scheme = HttpScheme::Https;
    manager
        .replace_many(vec![update("first", 2, allowed.clone())])
        .await
        .unwrap();
    let socket = TcpSocket::new_v4().unwrap();
    socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
    let mut stream = socket
        .connect(([127, 0, 0, 1], listen).into())
        .await
        .unwrap();
    stream
        .write_all(b"CONNECT 127.0.0.1:443 HTTP/1.1\r\nHost: 127.0.0.1:443\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        response.push(stream.read_u8().await.unwrap());
    }
    assert!(response.starts_with(b"HTTP/1.1 200"));
    // Leave the upgraded task waiting for TLS, after the outer HTTP task completes.
    tokio::task::yield_now().await;
    allowed.http.as_mut().unwrap().rules.clear();
    manager
        .replace_many(vec![update("first", 2, allowed)])
        .await
        .unwrap();
    let mut byte = [0];
    assert_eq!(stream.try_read(&mut byte).unwrap(), 0);
}
