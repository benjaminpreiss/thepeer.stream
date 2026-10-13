use super::*;
use crate::{
    http::{host::Counts, inspector::HttpInspector},
    test_transport::Fixture,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const NAME: &str = "thepeer-00112233445566778899aabbccddeeff.localhost";

#[test]
fn browser_hostname_requires_exact_prefix_and_128_bit_lowercase_hex_nonce() {
    assert!(valid_hostname(NAME));
    for invalid in [
        "localhost".to_owned(),
        NAME.replace("thepeer-", "other-"),
        NAME.replace("thepeer-", "THEPEER-"),
        NAME.replace("0011", "A011"),
        NAME.replace("0011", "g011"),
        format!("thepeer-{}.localhost", "a".repeat(31)),
        format!("thepeer-{}.localhost", "a".repeat(33)),
        format!("thepeer-{}.localhost", "é".repeat(16)),
        format!("{NAME}.evil.example"),
        format!("{NAME}:8080"),
    ] {
        assert!(!valid_hostname(&invalid));
    }
}

#[tokio::test(start_paused = true)]
async fn forwarding_tasks_outlive_old_stream_and_session_deadlines_but_stop_cancels() {
    let scope = Scope::new();
    let (mut browser, mut left) = tokio::io::duplex(128);
    let (mut right, mut target) = tokio::io::duplex(128);
    scope.spawn(async move {
        let _ = tokio::io::copy_bidirectional(&mut left, &mut right).await;
    });
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(601)).await;
    tokio::task::yield_now().await;
    browser.write_all(b"still connected").await.unwrap();
    let mut bytes = [0; 15];
    timeout(Duration::from_secs(1), target.read_exact(&mut bytes))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&bytes, b"still connected");
    scope.finish().await;
    assert_eq!(browser.read(&mut bytes).await.unwrap(), 0);
    assert_eq!(target.read(&mut bytes).await.unwrap(), 0);
}

#[tokio::test]
async fn real_browser_http_crosses_authenticated_quic_to_fixed_target() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    let (inspector, token) = HttpInspector::new(target_port).unwrap();
    let mut fixture = Fixture::application_loopback().await.unwrap();
    let endpoint = fixture.backend.endpoint.clone();
    let backend = tokio::spawn(async move {
        let mut counts = Counts::default();
        let result = serve_target(&endpoint, inspector, target_port, &mut counts).await;
        (result, counts)
    });
    let target_task = tokio::spawn(async move {
        let (mut stream, _) = target.accept().await.unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            header.push(stream.read_u8().await.unwrap());
            assert!(header.len() < 16384);
        }
        assert!(header.starts_with(b"GET /app.js?test=1 HTTP/1.1\r\n"));
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: 19\r\nConnection: close\r\n\r\nglobalThis.ok=true;").await.unwrap();
    });
    let session = Session::open(
        "127.0.0.1:0".parse().unwrap(),
        fixture.destination,
        fixture.backend.public_pin,
        &token,
    )
    .await
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let authority = format!("{NAME}:{}", addr.port());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let gateway = tokio::spawn(drive(listener, session, NAME.to_owned(), async {
        let _ = stopped.await;
    }));
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{addr}/app.js?test=1"))
        .header("Host", &authority)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(!response.headers().contains_key("content-security-policy"));
    assert_eq!(response.text().await.unwrap(), "globalThis.ok=true;");
    stop.send(()).unwrap();
    assert!(gateway.await.unwrap().is_ok());
    let (result, counts) = backend.await.unwrap();
    result.unwrap();
    assert!(counts.authorized);
    assert_eq!(counts.fetched, 1);
    target_task.await.unwrap();
    fixture.shutdown().await;
}

struct Running {
    fixture: Fixture,
    addr: SocketAddr,
    authority: String,
    stop: tokio::sync::oneshot::Sender<()>,
    gateway: tokio::task::JoinHandle<Result<&'static str>>,
    backend: tokio::task::JoinHandle<(Result<()>, Counts)>,
}
impl Running {
    async fn start(port: u16) -> Self {
        Self::start_target(crate::target::PreparedTarget::loopback(port)).await
    }
    async fn start_target(target: crate::target::PreparedTarget) -> Self {
        let (inspector, token) = HttpInspector::new(target.target.port()).unwrap();
        let fixture = Fixture::application_loopback().await.unwrap();
        let endpoint = fixture.backend.endpoint.clone();
        let backend = tokio::spawn(async move {
            let mut counts = Counts::default();
            let result = serve_target_inner(&endpoint, None, inspector, target, &mut counts).await;
            (result, counts)
        });
        let session = Session::open(
            "127.0.0.1:0".parse().unwrap(),
            fixture.destination,
            fixture.backend.public_pin,
            &token,
        )
        .await
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let authority = format!("{NAME}:{}", addr.port());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let gateway = tokio::spawn(drive(listener, session, NAME.to_owned(), async {
            let _ = stopped.await;
        }));
        Self {
            fixture,
            addr,
            authority,
            stop,
            gateway,
            backend,
        }
    }
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .request(method, format!("http://{}{path}", self.addr))
            .header("Host", &self.authority)
            .header("Origin", format!("http://{}", self.authority))
    }
    async fn finish(mut self) -> Counts {
        self.stop.send(()).unwrap();
        self.gateway.await.unwrap().unwrap();
        assert!(TcpStream::connect(self.addr).await.is_err());
        let (result, counts) = self.backend.await.unwrap();
        result.unwrap();
        self.fixture.shutdown().await;
        counts
    }
}
async fn header(socket: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    while !buf.ends_with(b"\r\n\r\n") {
        buf.push(socket.read_u8().await.unwrap());
        assert!(buf.len() < 16384);
    }
    String::from_utf8(buf).unwrap()
}

#[tokio::test]
async fn application_csp_is_preserved_without_injecting_a_tunnel_policy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let running = Running::start(listener.local_addr().unwrap().port()).await;
    let policies = [
        "default-src 'self'; media-src https://video.example",
        "script-src 'none'",
    ];
    let task = tokio::spawn(async move {
        for headers in [
            String::new(),
            format!(
                "Content-Security-Policy: {}\r\nContent-Security-Policy: {}\r\nContent-Security-Policy-Report-Only: default-src 'none'\r\n",
                policies[0], policies[1]
            ),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            header(&mut socket).await;
            socket.write_all(format!("HTTP/1.1 200 OK\r\n{headers}Content-Length: 2\r\nConnection: close\r\n\r\nok").as_bytes()).await.unwrap();
        }
    });
    let response = running
        .request(reqwest::Method::GET, "/")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(!response.headers().contains_key("content-security-policy"));
    assert!(
        !response
            .headers()
            .contains_key("content-security-policy-report-only")
    );
    assert_eq!(response.text().await.unwrap(), "ok");
    let response = running
        .request(reqwest::Method::GET, "/")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let actual: Vec<_> = response
        .headers()
        .get_all("content-security-policy")
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect();
    assert_eq!(actual, policies);
    assert_eq!(
        response.headers()["content-security-policy-report-only"],
        "default-src 'none'"
    );
    assert_eq!(response.text().await.unwrap(), "ok");
    task.await.unwrap();
    running.finish().await;
}

#[tokio::test]
async fn custom_local_name_maps_host_without_inventing_origin_and_rewrites_only_its_redirects() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let target_name = format!("custom.test:{}", addr.port());
    let target: crate::target::Target = target_name.parse().unwrap();
    let prepared = target
        .prepare_with(&CancellationToken::new(), async { Ok(vec![addr]) })
        .await
        .unwrap();
    let running = Running::start_target(prepared).await;
    let task = tokio::spawn(async move {
        for location in [
            format!("http://{target_name}?next=1#section"),
            "next?return=http://example.invalid/".to_owned(),
            format!("http://{target_name}.evil.invalid/path"),
            format!("https://{target_name}/path"),
            "//example.invalid/path".to_owned(),
            "/\t/example.invalid/path".to_owned(),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let h = header(&mut socket).await.to_ascii_lowercase();
            assert!(h.contains(&format!("\r\nhost: {target_name}\r\n")));
            assert!(!h.contains("\r\norigin:"));
            socket.write_all(format!("HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: {location}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        }
    });
    for index in 0..6 {
        let response = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(format!("http://{}/", running.addr))
            .header("Host", &running.authority)
            .send()
            .await
            .unwrap();
        if index == 0 {
            assert_eq!(response.status(), 302);
            assert_eq!(
                response.headers()["location"],
                format!("http://{}/?next=1#section", running.authority)
            );
        } else if index == 1 {
            assert_eq!(response.status(), 302);
            assert_eq!(
                response.headers()["location"],
                "next?return=http://example.invalid/"
            );
        } else {
            assert_eq!(response.status(), 502);
        }
    }
    task.await.unwrap();
    assert_eq!(running.finish().await.fetched, 6);
}

#[tokio::test]
async fn legacy_or_invalid_authority_acknowledgements_fail_before_application_streams() {
    for ack in [
        serde_json::json!({"v":1,"type":"application-authorized","targetPort":4321}),
        serde_json::json!({"v":2,"type":"application-authorized","targetAuthority":"https://localhost:4321"}),
        serde_json::json!({"v":2,"type":"application-authorized","targetAuthority":"192.0.2.1:4321"}),
        serde_json::json!({"v":2,"type":"application-authorized","targetAuthority":"LOCALHOST:4321"}),
        serde_json::json!({"v":2,"type":"application-authorized","targetAuthority":"localhost:4321","extra":true}),
    ] {
        let mut fixture = Fixture::application_loopback().await.unwrap();
        let endpoint = fixture.backend.endpoint.clone();
        let backend = tokio::spawn(async move {
            let connection = endpoint.accept().await.unwrap().await.unwrap();
            let (mut send, mut recv) = connection.accept_bi().await.unwrap();
            let auth = read_frame(&mut recv).await.unwrap().unwrap();
            drop(auth);
            write_frame(&mut send, &serde_json::to_vec(&ack).unwrap())
                .await
                .unwrap();
            send.finish().unwrap();
            assert!(
                timeout(Duration::from_secs(2), connection.accept_bi())
                    .await
                    .unwrap()
                    .is_err()
            );
        });
        let result = Session::open(
            "127.0.0.1:0".parse().unwrap(),
            fixture.destination,
            fixture.backend.public_pin,
            &"00".repeat(32),
        )
        .await;
        assert!(matches!(result, Err(Failure("application-auth-ack"))));
        backend.await.unwrap();
        fixture.shutdown().await;
    }
}

#[tokio::test]
async fn validated_browser_authority_is_mapped_and_spoofed_forwarding_headers_are_removed() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    let running = Running::start(port).await;
    let task = tokio::spawn(async move {
        let (mut socket, _) = target.accept().await.unwrap();
        let h = header(&mut socket).await.to_ascii_lowercase();
        assert!(h.contains(&format!("\r\nhost: localhost:{port}\r\n")));
        assert!(h.contains(&format!("\r\norigin: http://localhost:{port}\r\n")));
        assert!(!h.contains("forwarded"));
        assert!(!h.contains("x-real-ip"));
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await
            .unwrap();
    });
    let response = running
        .request(reqwest::Method::POST, "/write")
        .header("Forwarded", "host=attacker.invalid")
        .header("X-Forwarded-Host", "attacker.invalid")
        .header("X-Forwarded-For", "192.0.2.1")
        .header("X-Forwarded-Proto", "https")
        .header("X-Real-IP", "192.0.2.1")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
    task.await.unwrap();
    running.finish().await;
}

#[tokio::test]
async fn ipv6_only_localhost_target_and_redirect_cross_authenticated_quic() {
    let target = TcpListener::bind("[::1]:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    // Ensure this fixture really has no IPv4 listener at the same port.
    assert!(
        TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .is_err()
    );
    let configured = format!("[::1]:{port}")
        .parse::<crate::target::Target>()
        .unwrap();
    let prepared = configured.prepare(&CancellationToken::new()).await.unwrap();
    let running = Running::start_target(prepared).await;
    let target_task = tokio::spawn(async move {
        for reply in [
            format!(
                "HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: http://[::1]:{port}/hello\r\nConnection: close\r\n\r\n"
            ),
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nipv6".to_owned(),
        ] {
            let (mut socket, _) = target.accept().await.unwrap();
            header(&mut socket).await;
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let response = running
        .request(reqwest::Method::GET, "/")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 302);
    assert_eq!(
        response.headers()["location"],
        format!("http://{}/hello", running.authority)
    );
    assert_eq!(
        running
            .request(reqwest::Method::GET, "/hello")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ipv6"
    );
    target_task.await.unwrap();
    assert_eq!(running.finish().await.fetched, 2);
}

#[tokio::test]
async fn resolver_order_is_preserved_and_http_failure_is_not_replayed() {
    for (respond, ipv6_first) in [(true, false), (false, false), (true, true), (false, true)] {
        let ipv4 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = ipv4.local_addr().unwrap().port();
        let ipv6 = TcpListener::bind((std::net::Ipv6Addr::LOCALHOST, port))
            .await
            .unwrap();
        let (first, second) = if ipv6_first {
            (ipv6, ipv4)
        } else {
            (ipv4, ipv6)
        };
        let addresses = vec![first.local_addr().unwrap(), second.local_addr().unwrap()];
        let target: crate::target::Target = format!("custom.test:{port}").parse().unwrap();
        let prepared = target
            .prepare_with(&CancellationToken::new(), async { Ok(addresses) })
            .await
            .unwrap();
        let running = Running::start_target(prepared).await;
        let task = tokio::spawn(async move {
            let (mut socket, _) = timeout(Duration::from_secs(5), first.accept())
                .await
                .unwrap()
                .unwrap();
            assert!(header(&mut socket).await.starts_with("POST /write "));
            if respond {
                socket
                    .write_all(
                        b"HTTP/1.1 418 Teapot\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            }
            // Closing an established target without a response must not retry
            // the potentially mutating request against another local service.
        });
        let response = running
            .request(reqwest::Method::POST, "/write")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), if respond { 418 } else { 502 });
        task.await.unwrap();
        assert!(
            timeout(Duration::from_millis(100), second.accept())
                .await
                .is_err()
        );
        assert_eq!(running.finish().await.fetched, 1);
    }
}

#[tokio::test]
async fn authenticated_quic_session_serves_more_than_the_old_512_stream_budget() {
    timeout(Duration::from_secs(60), async {
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let running = Running::start(target.local_addr().unwrap().port()).await;
        let target_task = tokio::spawn(async move {
            for _ in 0..520 {
                let (mut socket, _) = target.accept().await.unwrap();
                header(&mut socket).await;
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .await
                    .unwrap();
            }
        });
        for _ in 0..520 {
            let response = running
                .request(reqwest::Method::GET, "/")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.text().await.unwrap(), "ok");
        }
        target_task.await.unwrap();
        assert_eq!(running.finish().await.fetched, 520);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn binary_post_and_sse_are_streamed_without_blocking_parallel_http() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let running = Running::start(target.local_addr().unwrap().port()).await;
    let release = Arc::new(tokio::sync::Notify::new());
    let signal = release.clone();
    let target_task = tokio::spawn(async move {
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..3 {
            let (mut socket, _) = target.accept().await.unwrap();
            let signal = signal.clone();
            tasks.spawn(async move {
                let h = header(&mut socket).await;
                if h.starts_with("POST /upload ") {
                    assert!(h.to_ascii_lowercase().contains("content-length: 65536"));
                    let mut body = vec![0; 65536]; socket.read_exact(&mut body).await.unwrap();
                    assert!(body.iter().enumerate().all(|(i,b)| *b == (i % 256) as u8));
                    socket.write_all(b"HTTP/1.1 201 Created\r\nContent-Type: application/octet-stream\r\nContent-Length: 65536\r\nConnection: close\r\n\r\n").await.unwrap();
                    socket.write_all(&body).await.unwrap();
                } else if h.starts_with("GET /events ") {
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n9\r\ndata: 1\n\n\r\n").await.unwrap();
                    signal.notified().await;
                    socket.write_all(b"9\r\ndata: 2\n\n\r\n0\r\n\r\n").await.unwrap();
                } else {
                    assert!(h.starts_with("GET /parallel "));
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
                    signal.notify_one();
                }
            });
        }
        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }
    });
    let bytes: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
    let response = running
        .request(reqwest::Method::POST, "/upload")
        .body(bytes.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    assert_eq!(response.bytes().await.unwrap().as_ref(), bytes);
    let mut events = running
        .request(reqwest::Method::GET, "/events")
        .send()
        .await
        .unwrap();
    let mut first = Vec::new();
    timeout(Duration::from_secs(5), async {
        while first.len() < 9 {
            first.extend(events.chunk().await.unwrap().unwrap());
        }
    })
    .await
    .unwrap();
    assert_eq!(first, b"data: 1\n\n");
    assert_eq!(
        running
            .request(reqwest::Method::GET, "/parallel")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(events.text().await.unwrap(), "data: 2\n\n");
    target_task.await.unwrap();
    assert_eq!(running.finish().await.fetched, 3);
}

#[tokio::test]
async fn websocket_upgrade_and_binary_frames_cross_real_quic() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let running = Running::start(target.local_addr().unwrap().port()).await;
    let target_task = tokio::spawn(async move {
        let (mut socket, _) = target.accept().await.unwrap();
        assert!(
            header(&mut socket)
                .await
                .starts_with("GET /socket HTTP/1.1")
        );
        socket.write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n").await.unwrap();
        let mut frame = [0; 10];
        socket.read_exact(&mut frame).await.unwrap();
        assert_eq!(frame, [0x82, 0x84, 1, 2, 3, 4, 1, 253, 124, 132]);
        socket
            .write_all(&[0x82, 4, 0, 255, 127, 128])
            .await
            .unwrap();
    });
    let mut browser = TcpStream::connect(running.addr).await.unwrap();
    browser.write_all(format!("GET /socket HTTP/1.1\r\nHost: {}\r\nOrigin: http://{}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n", running.authority, running.authority).as_bytes()).await.unwrap();
    let response = timeout(Duration::from_secs(5), header(&mut browser))
        .await
        .unwrap();
    assert!(response.starts_with("HTTP/1.1 101"));
    browser
        .write_all(&[0x82, 0x84, 1, 2, 3, 4, 1, 253, 124, 132])
        .await
        .unwrap();
    let mut frame = [0; 6];
    timeout(Duration::from_secs(5), browser.read_exact(&mut frame))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frame, [0x82, 4, 0, 255, 127, 128]);
    target_task.await.unwrap();
    drop(browser);
    assert_eq!(running.finish().await.fetched, 1);
}

#[tokio::test]
async fn wrong_pin_wrong_grant_and_wrong_protocol_never_reach_target() {
    for wrong in ["pin", "grant", "protocol"] {
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = target.local_addr().unwrap().port();
        let (inspector, token) = HttpInspector::new(port).unwrap();
        let mut fixture = Fixture::application_loopback().await.unwrap();
        let endpoint = fixture.backend.endpoint.clone();
        let backend = tokio::spawn(async move {
            let mut counts = Counts::default();
            let result = serve_target(&endpoint, inspector, port, &mut counts).await;
            (result, counts)
        });
        let mut pin = fixture.backend.public_pin;
        if wrong == "pin" {
            pin[0] ^= 1;
        }
        if wrong == "protocol" {
            let (endpoint, rejected) =
                client_endpoint("127.0.0.1:0".parse().unwrap(), pin, WireMode::Viewer).unwrap();
            assert!(
                connect(&endpoint, fixture.destination, &rejected)
                    .await
                    .is_err()
            );
            endpoint.close(0u32.into(), b"test complete");
        } else {
            let bad = "0".repeat(64);
            let result = timeout(
                Duration::from_secs(5),
                Session::open(
                    "127.0.0.1:0".parse().unwrap(),
                    fixture.destination,
                    pin,
                    if wrong == "grant" { &bad } else { &token },
                ),
            )
            .await
            .unwrap();
            assert!(result.is_err());
        }
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        let (result, counts) = timeout(Duration::from_secs(5), backend)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_err());
        assert!(!counts.authorized);
        assert_eq!(counts.fetched, 0);
        assert_eq!(counts.failed, 1);
        fixture.shutdown().await;
    }
}

#[tokio::test]
async fn every_keepalive_request_checks_origin_and_host_before_any_target_access() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    let running = Running::start(port).await;
    let mut browser = TcpStream::connect(running.addr).await.unwrap();
    for extra in [
        "Origin: https://evil.invalid\r\n",
        "Origin: null\r\n",
        "Sec-Fetch-Site: cross-site\r\n",
        "Host: other.localhost:1234\r\n",
        "Origin: http://wrong.localhost\r\n",
    ] {
        browser
            .write_all(
                format!(
                    "GET /private HTTP/1.1\r\nHost: {}\r\n{extra}\r\n",
                    running.authority
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let head = header(&mut browser).await;
        assert!(head.starts_with("HTTP/1.1 403"));
        let length: usize = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length: "))
            .unwrap()
            .parse()
            .unwrap();
        let mut body = vec![0; length];
        browser.read_exact(&mut body).await.unwrap();
    }
    assert_eq!(
        running
            .request(reqwest::Method::POST, "/")
            .header("Origin", "null")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        running
            .request(reqwest::Method::POST, "/")
            .header("Content-Length", REQUEST_BYTES + 1)
            .body("")
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    assert!(
        timeout(Duration::from_millis(100), target.accept())
            .await
            .is_err()
    );
    drop(browser);
    assert_eq!(running.finish().await.fetched, 0);
}

#[tokio::test]
async fn target_status_host_only_cookies_and_local_redirects_survive_but_external_redirects_do_not()
{
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    let running = Running::start(port).await;
    let task = tokio::spawn(async move {
        for response in [
            "HTTP/1.1 418 Teapot\r\nContent-Length: 0\r\nSet-Cookie: a=public; Path=/; SameSite=Strict\r\nSet-Cookie: b=fixture; HttpOnly; Path=/; SameSite=Strict\r\n\r\n".to_owned(),
            format!("HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: http://localhost:{port}/two?q=1\r\n\r\n"),
            format!("HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: http://[::1]:{}/\r\n\r\n", if port == 65535 { 1 } else { port + 1 }),
            "HTTP/1.1 302 Found\r\nContent-Length: 0\r\nLocation: https://example.invalid/\r\n\r\n".to_owned(),
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nSet-Cookie: a=public; Domain=localhost\r\n\r\n".to_owned(),
        ] {
            let (mut socket, _) = target.accept().await.unwrap(); header(&mut socket).await;
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let response = running
        .request(reqwest::Method::GET, "/status")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 418);
    assert_eq!(response.headers().get_all("set-cookie").iter().count(), 2);
    let response = running
        .request(reqwest::Method::GET, "/redirect")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 302);
    assert_eq!(
        response.headers()["location"],
        format!("http://{}/two?q=1", running.authority)
    );
    for path in ["/wrong-ipv6-port", "/external", "/domain-cookie"] {
        assert_eq!(
            running
                .request(reqwest::Method::GET, path)
                .send()
                .await
                .unwrap()
                .status(),
            502
        );
    }
    task.await.unwrap();
    assert_eq!(running.finish().await.fetched, 5);
}

#[tokio::test]
async fn dropping_owner_cancels_active_stream_tasks_and_closes_the_origin() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let running = Running::start(target.local_addr().unwrap().port()).await;
    let target_task = tokio::spawn(async move {
        let (mut socket, _) = target.accept().await.unwrap();
        header(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n9\r\ndata: 1\n\n\r\n").await.unwrap();
        std::future::pending::<()>().await;
        drop(socket);
    });
    let mut response = running
        .request(reqwest::Method::GET, "/events")
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.chunk().await.unwrap().unwrap().as_ref(),
        b"data: 1\n\n"
    );
    assert!(
        timeout(Duration::from_millis(50), response.chunk())
            .await
            .is_err()
    );
    running.gateway.abort();
    assert!(running.gateway.await.unwrap_err().is_cancelled());
    let ended = timeout(Duration::from_secs(3), response.chunk())
        .await
        .unwrap();
    assert!(ended.is_err() || ended.unwrap().is_none());
    assert!(TcpStream::connect(running.addr).await.is_err());
    let (result, counts) = timeout(Duration::from_secs(3), running.backend)
        .await
        .unwrap()
        .unwrap();
    result.unwrap();
    assert!(counts.authorized);
    target_task.abort();
    let _ = target_task.await;
    let mut fixture = running.fixture;
    fixture.shutdown().await;
}
