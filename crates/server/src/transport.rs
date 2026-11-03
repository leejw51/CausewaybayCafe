//! HTTP/3 serves the same router and Cap'n Proto contract over QUIC.
use axum::{body::Body, Router};
use bytes::Buf;
use futures_util::StreamExt;
use std::{io::BufReader, sync::Arc};
use tower::ServiceExt;
pub async fn serve_h3(
    router: Router,
    cert: &str,
    key: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let certs = rustls_pemfile::certs(&mut BufReader::new(std::fs::File::open(cert)?))
        .collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(std::fs::File::open(key)?))?
        .ok_or("Missing TLS key")?;
    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    tls.alpn_protocols = vec![b"h3".to_vec()];
    let config = quinn::ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls)?,
    ));
    let addr = std::env::var("CAFE_HTTP3_ADDR")
        .unwrap_or("0.0.0.0:8789".into())
        .parse()?;
    let endpoint = quinn::Endpoint::server(config, addr)?;
    println!("HTTP/3 listening on {addr}");
    while let Some(incoming) = endpoint.accept().await {
        let router = router.clone();
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else {
                return;
            };
            let Ok(mut conn) = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await
            else {
                return;
            };
            while let Ok(Some(resolver)) = conn.accept().await {
                let router = router.clone();
                tokio::spawn(async move {
                    let Ok((request, mut stream)) = resolver.resolve_request().await else {
                        return;
                    };
                    let mut data = Vec::new();
                    loop {
                        let mut chunk = match stream.recv_data().await {
                            Ok(Some(chunk)) => chunk,
                            Ok(None) => break,
                            Err(_) => return,
                        };
                        if data.len() + chunk.remaining() > 8 * 1024 * 1024 {
                            return;
                        }
                        let n = chunk.remaining();
                        data.extend_from_slice(&chunk.copy_to_bytes(n));
                    }
                    let (parts, _) = request.into_parts();
                    let request = http::Request::from_parts(parts, Body::from(data));
                    if let Ok(response) = router.oneshot(request).await {
                        let (parts, body) = response.into_parts();
                        // Send headers and chunks immediately, including never-ending SSE bodies.
                        let response = http::Response::from_parts(parts, ());
                        if stream.send_response(response).await.is_err() {
                            return;
                        }
                        let mut body = body.into_data_stream();
                        while let Some(chunk) = body.next().await {
                            let Ok(chunk) = chunk else {
                                return;
                            };
                            if stream.send_data(chunk).await.is_err() {
                                return;
                            }
                        }
                        let _ = stream.finish().await;
                    }
                });
            }
        });
    }
    Ok(())
}
