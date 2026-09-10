use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

#[tokio::test]
async fn upload_preserves_binary_content_and_authentication() {
    assert_upload("PUT", "/v1/kernels/vmlinux/content").await;
    assert_upload("POST", "/v1/assets?alias=config&filename=app.json").await;
}

async fn assert_upload(method: &'static str, path: &'static str) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let content = vec![0, 0xff, b'\n', b'\r', b'K'];
    let expected = content.clone();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
            assert!(header.len() < 8192);
        }
        let header = String::from_utf8(header).unwrap().to_ascii_lowercase();
        assert!(header.starts_with(&format!(
            "{} {path} http/1.1\r\n",
            method.to_ascii_lowercase()
        )));
        assert!(header.contains("\r\nauthorization: bearer upload-test-token\r\n"));
        assert!(header.contains("\r\ncontent-type: application/octet-stream\r\n"));
        assert!(header.contains("\r\ncontent-length: 5\r\n"));
        let mut body = vec![0; expected.len()];
        stream.read_exact(&mut body).unwrap();
        assert_eq!(body, expected);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\nConnection: close\r\n\r\n{\"name\":\"vmlinux\"}")
            .unwrap();
    });
    let client = firemage_client::Client::new(
        &format!("http://{address}"),
        None,
        Some("upload-test-token".into()),
    )
    .unwrap();
    let response: serde_json::Value = match method {
        "PUT" => client.put_bytes(path, content).await.unwrap(),
        "POST" => client.post_bytes(path, content).await.unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(response["name"], "vmlinux");
    server.join().unwrap();
}
