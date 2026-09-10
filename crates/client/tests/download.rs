use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};

#[tokio::test]
async fn binary_download_checks_declared_and_streamed_limits() {
    for (response, should_succeed) in [
        (b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\n\x00\xffK".as_slice(), true),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n1234".as_slice(), false),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\n12\r\n2\r\n34\r\n0\r\n\r\n".as_slice(), false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
                assert!(header.len() < 8192);
            }
            let header = String::from_utf8(header).unwrap().to_ascii_lowercase();
            assert!(header.starts_with("get /v1/assets/id/content http/1.1\r\n"));
            assert!(header.contains("authorization: bearer download-test-token\r\n"));
            stream.write_all(response).unwrap();
        });
        let client = firemage_client::Client::new(&format!("http://{address}"), None, Some("download-test-token".into())).unwrap();
        let result = client.get_bytes("/v1/assets/id/content", 3).await;
        if should_succeed {
            assert_eq!(result.unwrap(), [0, 0xff, b'K']);
        } else {
            assert!(result.unwrap_err().to_string().contains("exceeds 3 bytes"));
        }
        server.join().unwrap();
    }
}
