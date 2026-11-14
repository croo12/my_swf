use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
};

const TOKEN: &str = "test-fixture-token-not-a-credential";

#[test]
fn repository_rejects_unsafe_base_urls_before_sending_credentials() {
    for base in [
        "http://example.com",
        "https://user:password@example.com",
        "https://example.com/subpath",
        "https://example.com/?query=1",
        "https://example.com/#fragment",
        "file:///repository",
    ] {
        assert!(
            fetch(base, TOKEN).err().unwrap().contains("HTTPS"),
            "{base}"
        );
    }
    assert!(
        fetch("https://example.com", "short")
            .err()
            .unwrap()
            .contains("24자")
    );
}

#[test]
fn repository_does_not_follow_redirects_and_sends_the_expected_authenticated_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0, "Client closed before sending HTTP headers");
            bytes.extend_from_slice(&buffer[..count]);
        }
        stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        String::from_utf8(bytes).unwrap()
    });
    let error = fetch(&base, TOKEN).err().unwrap();
    let request = server.join().unwrap().to_ascii_lowercase();
    assert!(error.contains("302"), "{error}");
    assert!(request.starts_with("get /api/repository/tree http/1.1\r\n"));
    assert!(request.contains(&format!("authorization: bearer {TOKEN}\r\n")));
}
