use firemage_egress_policy::{
    EgressPolicy, HttpProxy, HttpRule, HttpScheme, TcpTunnel, ValueSource,
};
use firemage_egress_proxy::EgressManager;
use firemage_request_signing::SecretResolver;
use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpSocket},
};

struct Secrets;
#[async_trait::async_trait]
impl SecretResolver for Secrets {
    async fn resolve(&self, name: &str) -> anyhow::Result<String> {
        anyhow::ensure!(name == "api-key", "unknown secret");
        Ok("injected-key".into())
    }
}
fn port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn rule(port: u16, scheme: HttpScheme) -> HttpRule {
    HttpRule {
        host: "localhost".into(),
        port,
        scheme,
        methods: vec!["GET".into()],
        path_prefix: "/allowed".into(),
        allowed_ips: vec!["127.0.0.0/8".parse().unwrap()],
        headers: BTreeMap::new(),
        signing: None,
    }
}
fn policy(port: u16, rule: HttpRule) -> EgressPolicy {
    EgressPolicy {
        http: Some(HttpProxy {
            port,
            rules: vec![rule],
            upstream_ca_pem: None,
        }),
        ..Default::default()
    }
}
fn client(proxy: u16, ca: Option<String>, guest: u8) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{proxy}")).unwrap())
        .local_address(IpAddr::V4(Ipv4Addr::new(127, 0, 0, guest)))
        .timeout(std::time::Duration::from_secs(5));
    if let Some(ca) = ca {
        builder =
            builder.add_root_certificate(reqwest::Certificate::from_pem(ca.as_bytes()).unwrap());
    }
    builder.build().unwrap()
}
async fn origin() -> (u16, tokio::sync::mpsc::UnboundedReceiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    headers.push(stream.read_u8().await.unwrap());
                }
                tx.send(String::from_utf8(headers).unwrap()).unwrap();
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .await
                    .unwrap();
            });
        }
    });
    (port, rx)
}
async fn register(manager: &EgressManager, name: &str, guest: u8, policy: EgressPolicy) {
    manager
        .register(
            name,
            Ipv4Addr::new(127, 0, 0, guest),
            Ipv4Addr::LOCALHOST,
            policy,
            Arc::new(Secrets),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn http_authority_rules_credentials_and_private_dns() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let (origin, mut requests) = origin().await;
    let proxy = port();
    let mut allowed = rule(origin, HttpScheme::Http);
    allowed.headers.insert(
        "Authorization".into(),
        ValueSource::Secret {
            secret: "api-key".into(),
            prefix: "Bearer ".into(),
        },
    );
    register(&manager, "one", 2, policy(proxy, allowed.clone())).await;
    let client = client(proxy, None, 2);
    let url = format!("http://localhost:{origin}/allowed");
    assert_eq!(
        client
            .get(&url)
            .header("Authorization", "stolen")
            .header("Proxy-Authorization", "hidden")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ok"
    );
    let headers = requests.recv().await.unwrap().to_lowercase();
    assert!(headers.contains("authorization: bearer injected-key"));
    assert!(!headers.contains("stolen"));
    assert!(!headers.contains("proxy-authorization"));
    for request in [
        client.get(format!("http://localhost:{origin}/denied")),
        client.post(&url),
        client.get(&url).header("Host", "other.example"),
        client.get(format!("http://127.0.0.1:{origin}/allowed")),
    ] {
        assert_eq!(request.send().await.unwrap().status(), 403);
    }
    manager.unregister("one").await;
    allowed.allowed_ips.clear();
    register(&manager, "two", 3, policy(proxy, allowed)).await;
    assert_eq!(
        self::client(proxy, None, 3)
            .get(&url)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
}

#[tokio::test]
async fn https_is_intercepted_and_verified_with_persisted_ca() {
    use tokio_rustls::{
        TlsAcceptor,
        rustls::{self, pki_types::PrivatePkcs8KeyDer},
    };
    let generated = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let origin_ca = generated.cert.pem();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![generated.cert.der().clone()],
        PrivatePkcs8KeyDer::from(generated.key_pair.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                if let Ok(mut stream) = acceptor.accept(stream).await {
                    let mut headers = Vec::new();
                    while !headers.ends_with(b"\r\n\r\n") {
                        let Ok(byte) = stream.read_u8().await else {
                            return;
                        };
                        headers.push(byte);
                    }
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nsecure")
                        .await;
                }
            });
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let ca = manager.ca_certificate_pem();
    let proxy = port();
    let mut config = policy(proxy, rule(origin, HttpScheme::Https));
    config.http.as_mut().unwrap().upstream_ca_pem = Some(origin_ca);
    register(&manager, "https", 2, config).await;
    let client = client(proxy, Some(ca.clone()), 2);
    assert_eq!(
        client
            .get(format!("https://localhost:{origin}/allowed"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "secure"
    );
    assert_eq!(
        client
            .get(format!("https://localhost:{origin}/denied"))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .post(format!("https://localhost:{origin}/allowed"))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert!(
        self::client(proxy, None, 2)
            .get(format!("https://localhost:{origin}/allowed"))
            .send()
            .await
            .is_err()
    );
    manager.unregister("https").await;
    drop(manager);
    assert_eq!(
        EgressManager::new(directory.path())
            .await
            .unwrap()
            .ca_certificate_pem(),
        ca
    );
}

#[tokio::test]
async fn shared_tcp_listeners_route_by_guest_and_revoke_active_streams() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let proxy = port();
    for (guest, marker) in [(2, b'A'), (3, b'B')] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(&[marker]).await.unwrap();
            let (mut read, mut write) = stream.split();
            let _ = tokio::io::copy(&mut read, &mut write).await;
        });
        register(
            &manager,
            &format!("guest-{guest}"),
            guest,
            EgressPolicy {
                tunnels: vec![TcpTunnel {
                    name: "echo".into(),
                    listen_port: proxy,
                    target_host: "127.0.0.1".into(),
                    target_port: target,
                    allowed_ips: vec![],
                }],
                ..Default::default()
            },
        )
        .await;
    }
    let mut streams = Vec::new();
    for (guest, marker) in [(2, b'A'), (3, b'B')] {
        let socket = TcpSocket::new_v4().unwrap();
        socket
            .bind(format!("127.0.0.{guest}:0").parse().unwrap())
            .unwrap();
        let mut stream = socket
            .connect(format!("127.0.0.1:{proxy}").parse().unwrap())
            .await
            .unwrap();
        assert_eq!(stream.read_u8().await.unwrap(), marker);
        stream.write_all(&[0, 255, 1, 128]).await.unwrap();
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(bytes, [0, 255, 1, 128]);
        streams.push(stream);
    }
    manager.unregister("guest-2").await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), streams[0].read_u8())
            .await
            .unwrap()
            .is_err()
    );
    streams[1].write_all(&[42]).await.unwrap();
    assert_eq!(streams[1].read_u8().await.unwrap(), 42);
    let mut unknown = tokio::net::TcpStream::connect(format!("127.0.0.1:{proxy}"))
        .await
        .unwrap();
    assert!(unknown.read_u8().await.is_err());
    manager.unregister("guest-3").await;
}

#[tokio::test]
async fn malformed_authority_and_connect_cannot_bypass_rules() {
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let proxy = port();
    register(
        &manager,
        "vm",
        2,
        policy(proxy, rule(443, HttpScheme::Https)),
    )
    .await;
    for request in [
        "CONNECT localhost:443 HTTP/1.1\r\nHost: forbidden.example:443\r\n\r\n",
        "CONNECT localhost:80 HTTP/1.1\r\nHost: localhost:80\r\n\r\n",
        "CONNECT user@localhost:443 HTTP/1.1\r\nHost: localhost:443\r\n\r\n",
        "CONNECT localhost:443 HTTP/1.1\r\nHost: localhost:443\r\nContent-Length: 4\r\n\r\nbody",
        "GET http://localhost:443/allowed HTTP/1.1\r\nHost: localhost:443\r\nHost: localhost:443\r\n\r\n",
    ] {
        let socket = TcpSocket::new_v4().unwrap();
        socket.bind("127.0.0.2:0".parse().unwrap()).unwrap();
        let mut stream = socket
            .connect(format!("127.0.0.1:{proxy}").parse().unwrap())
            .await
            .unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut result = [0; 64];
        let count =
            tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut result))
                .await
                .unwrap()
                .unwrap();
        let response = String::from_utf8_lossy(&result[..count]);
        assert!(
            response.starts_with("HTTP/1.1 403") || response.starts_with("HTTP/1.1 400"),
            "{response}"
        );
    }
}

#[tokio::test]
async fn event_stream_delivers_first_chunk_before_upstream_finishes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = listener.local_addr().unwrap().port();
    let (release, wait) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(stream.read_u8().await.unwrap());
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n9\r\ndata: 1\n\n\r\n").await.unwrap();
        wait.await.unwrap();
        stream
            .write_all(b"9\r\ndata: 2\n\n\r\n0\r\n\r\n")
            .await
            .unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(directory.path()).await.unwrap();
    let proxy = port();
    register(
        &manager,
        "stream",
        2,
        policy(proxy, rule(origin, HttpScheme::Http)),
    )
    .await;
    let mut response = client(proxy, None, 2)
        .get(format!("http://localhost:{origin}/allowed"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(1), response.chunk())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        "data: 1\n\n"
    );
    release.send(()).unwrap();
    assert_eq!(response.text().await.unwrap(), "data: 2\n\n");
}

#[tokio::test]
async fn persisted_ca_rejects_mismatched_private_key() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let manager = EgressManager::new(first.path()).await.unwrap();
    assert!(manager.ca_fingerprint().starts_with("SHA256:"));
    drop(manager);
    drop(EgressManager::new(second.path()).await.unwrap());
    std::fs::copy(
        second.path().join("egress-ca.key"),
        first.path().join("egress-ca.key"),
    )
    .unwrap();
    assert!(EgressManager::new(first.path()).await.is_err());
}
