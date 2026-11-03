use bytes::Buf;
#[tokio::test]
#[ignore = "opens localhost UDP sockets"]
async fn http3_serves_health_and_capnp_rpc() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    std::fs::write(&cert_path, cert.cert.pem()).unwrap();
    std::fs::write(&key_path, cert.key_pair.serialize_pem()).unwrap();
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let addr = format!("127.0.0.1:{port}");
    std::env::set_var("CAFE_HTTP3_ADDR", &addr);
    let app = cafe_server::init(":memory:", "", true).await.unwrap();
    let server = tokio::spawn(async move {
        cafe_server::transport::serve_h3(
            cafe_server::router(app),
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
        )
        .await
        .unwrap()
    });
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.cert.der().clone()).unwrap();
    let mut tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    tls.alpn_protocols = vec![b"h3".to_vec()];
    let config = quinn::ClientConfig::new(std::sync::Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls).unwrap(),
    ));
    let mut endpoint = quinn::Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
    endpoint.set_default_client_config(config);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let conn = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        endpoint
            .connect(addr.parse().unwrap(), "localhost")
            .unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    let (mut driver, mut sender) = h3::client::new(h3_quinn::Connection::new(conn))
        .await
        .unwrap();
    let drive = tokio::spawn(async move {
        std::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });
    let req = http::Request::builder()
        .uri(format!("https://localhost:{port}/health"))
        .body(())
        .unwrap();
    let mut stream = sender.send_request(req).await.unwrap();
    stream.finish().await.unwrap();
    assert_eq!(stream.recv_response().await.unwrap().status(), 200);
    let mut body = stream.recv_data().await.unwrap().unwrap();
    let len = body.remaining();
    assert_eq!(&body.copy_to_bytes(len)[..], b"ok");
    let request = cafe_shared::Rpc {
        method: "guest".into(),
        token: String::new(),
        payload: serde_json::json!({"table":7}),
    };
    let req = http::Request::builder()
        .method("POST")
        .uri(format!("https://localhost:{port}/api/rpc"))
        .header("Content-Type", "application/x-capnp")
        .body(())
        .unwrap();
    let mut stream = sender.send_request(req).await.unwrap();
    stream
        .send_data(bytes::Bytes::from(cafe_shared::encode(&request).unwrap()))
        .await
        .unwrap();
    stream.finish().await.unwrap();
    assert_eq!(stream.recv_response().await.unwrap().status(), 200);
    let mut bytes = vec![];
    while let Some(mut data) = stream.recv_data().await.unwrap() {
        let n = data.remaining();
        bytes.extend_from_slice(&data.copy_to_bytes(n));
    }
    assert_eq!(cafe_shared::decode(&bytes).unwrap().payload["ok"], true);
    let token = cafe_shared::decode(&bytes).unwrap().payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();
    let req = http::Request::builder()
        .method("POST")
        .uri(format!("https://localhost:{port}/api/events"))
        .header("Authorization", format!("Bearer {token}"))
        .body(())
        .unwrap();
    let mut events = sender.send_request(req).await.unwrap();
    events.finish().await.unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(2), events.recv_response())
            .await
            .unwrap()
            .unwrap()
            .status(),
        200
    );
    let mut event = tokio::time::timeout(std::time::Duration::from_secs(2), events.recv_data())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let n = event.remaining();
    assert!(String::from_utf8_lossy(&event.copy_to_bytes(n)).contains("data: refresh"));
    drive.abort();
    server.abort();
    endpoint.close(0u32.into(), b"test complete");
}
