# Local HTTPS and HTTP/3

Create a local self-signed CA and a server certificate signed by it:

```sh
make tls TLS_HOSTS='100.125.58.31 localhost'
make start
```

Add every LAN IP or DNS name used by guests to TLS_HOSTS. Reissue when addresses change. The existing CA is preserved, so devices do not need a new trust installation. Server certificates expire after 365 days. Files live in `~/.causewaybaycafe/tls` (or `CAFE_HOME/tls`); keys have owner-only permissions and are never served.

- HTTPS: `https://100.125.58.31:8792/` (TCP, HTTP/1.1 fallback)
- HTTP/3: same address and port, QUIC over UDP. HTTPS advertises `Alt-Svc: h3=":8792"`. Browser/network support decides whether HTTP/3 is selected.
- Public CA download: `http://100.125.58.31:8791/local-ca.cer`
- HTTP remains available on 8791 for existing test links.

On iPhone/iPad, download the CA certificate, install it in Settings → General → VPN & Device Management, then enable it under Settings → General → About → Certificate Trust Settings → Enable Full Trust for Root Certificates. Open the HTTPS address again and grant microphone permission. Only install this CA on your own test devices; remove it when testing ends. No trust-store changes are performed by the setup script.

Apple instructions: https://support.apple.com/en-gb/102390

A certificate warning bypass is not equivalent to installing trust. Real microphone support depends on Safari/OS permissions and speech recognition availability; HTTPS alone does not guarantee recognition.

`make test-http3` tests certificate validation, health, Cap’n Proto RPC, and streaming events over QUIC. Check deployment without disabling validation:

```sh
curl --cacert ~/.causewaybaycafe/tls/ca.pem https://100.125.58.31:8792/health
```

For removal, stop the café, move the TLS directory outside CAFE_HOME, and start again; without certificates the supervisor uses HTTP only.
