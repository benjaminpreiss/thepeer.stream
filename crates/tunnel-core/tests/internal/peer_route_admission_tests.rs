use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::UdpSocket;

#[path = "peer_route_contention_tests.rs"]
mod contention_tests;

async fn node(relay: bool) -> P2pEndpoint {
    P2pEndpoint::new(
        P2pConfig::builder()
            .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .nat(NatConfig {
                allow_loopback: true,
                enable_relay_service: relay,
                advertise_external_addresses: false,
                upnp: UpnpConfig::disabled(),
                ..NatConfig::default()
            })
            .build()
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn virtual_admission_bounds_unfinished_tls_and_stop_releases_owner() {
    timeout(Duration::from_secs(35), async {
        let relay = node(true).await;
        let client = node(false).await;
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bind = reservation.local_addr().unwrap();
        drop(reservation);
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stop = CancellationToken::new();
        let (mut helper, offer) = Helper::prepare_with_relay(HelperConfig {
            bind, target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(), observer: None,
        }, relay.local_addr().unwrap(), &stop).await.unwrap();
        let text = offer.encode().unwrap();
        let decoded = Offer::decode(&text).unwrap();
        let contact = decoded.relay_candidate().unwrap();
        drop((offer, text, decoded));
        let network_stop = CancellationToken::new();
        let mut forwarding = JoinSet::new();
        let mut attempts = Vec::new();
        for slot in 0..9 {
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let address = proxy.local_addr().unwrap();
            let client_addr = client.local_addr().unwrap();
            let replies = Arc::new(AtomicUsize::new(0));
            let count = replies.clone();
            let done = network_stop.clone();
            let (sent, arrived) = oneshot::channel();
            forwarding.spawn(async move {
                let mut sent = Some(sent);
                let mut packet = vec![0; 65536];
                loop {
                    let (len, source) = tokio::select! {
                        _ = done.cancelled() => break,
                        incoming = proxy.recv_from(&mut packet) => incoming.unwrap(),
                    };
                    if source == contact {
                        count.fetch_add(1, Ordering::SeqCst);
                        // Real TLS flights are lost, never fabricated. The ninth
                        // client receives its actual refusal (or faulty acceptance).
                        if slot == 8 { proxy.send_to(&packet[..len], client_addr).await.unwrap(); }
                    } else {
                        assert_eq!(source, client_addr);
                        proxy.send_to(&packet[..len], contact).await.unwrap();
                        if let Some(sent) = sent.take() { let _ = sent.send(()); }
                    }
                }
            });
            let attempt = client.begin_connect(address).unwrap();
            arrived.await.unwrap();
            if slot < 4 {
                timeout(Duration::from_secs(3), async {
                    while replies.load(Ordering::SeqCst) == 0 {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.expect("four real TLS handshakes must start");
            } else if slot < 8 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert_eq!(replies.load(Ordering::SeqCst), 0,
                    "queued virtual attempts must not start TLS or be refused early");
            } else {
                let result = timeout(Duration::from_secs(3), attempt).await
                    .expect("overflow must be refused, not silently queued");
                let refused = matches!(result,
                    Err(saorsa_transport::ConnectionError::ConnectionClosed(ref error))
                    if error.error_code == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED);
                assert!(refused, "ninth unfinished virtual attempt must receive CONNECTION_REFUSED");
                break;
            }
            attempts.push(attempt);
        }
        assert_eq!(helper.progress().admitted, 0);
        assert!(!helper.progress().authorized);
        assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        stop.cancel();
        timeout(Duration::from_secs(18), helper.wait()).await.unwrap().unwrap();
        helper.shutdown().await.unwrap();
        let _released = UdpSocket::bind(bind).await.unwrap();
        assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        for attempt in attempts { let _ = attempt.abort(); }
        network_stop.cancel();
        while let Some(done) = forwarding.join_next().await { done.unwrap(); }
        assert!(relay.is_running());
        timeout(Duration::from_secs(8), client.shutdown()).await.unwrap();
        timeout(Duration::from_secs(8), relay.shutdown()).await.unwrap();
    }).await.expect("bounded real virtual-admission fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn expired_virtual_handshakes_release_capacity_for_relay_browser_http() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    timeout(Duration::from_secs(45), async {
        let relay = node(true).await;
        let attacker = node(false).await;
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stop = CancellationToken::new();
        let (helper, mut offer) = Helper::prepare_with_relay(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: None,
            },
            relay.local_addr().unwrap(),
            &stop,
        )
        .await
        .unwrap();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.set_direct_candidate(quiet.local_addr().unwrap());
        let text = offer.encode().unwrap();
        let decoded = Offer::decode(&text).unwrap();
        drop((offer, text));
        let contact = decoded.relay_candidate().unwrap();
        let network_stop = CancellationToken::new();
        let mut forwarding = JoinSet::new();
        let mut attempts = Vec::new();
        for _ in 0..4 {
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let address = proxy.local_addr().unwrap();
            let source_addr = attacker.local_addr().unwrap();
            let done = network_stop.clone();
            let (started, response) = oneshot::channel();
            forwarding.spawn(async move {
                let mut started = Some(started);
                let mut packet = vec![0; 65536];
                loop {
                    let (len, source) = tokio::select! {
                        _ = done.cancelled() => break,
                        received = proxy.recv_from(&mut packet) => received.unwrap(),
                    };
                    if source == contact {
                        if let Some(started) = started.take() {
                            let _ = started.send(());
                        }
                    } else {
                        assert_eq!(source, source_addr);
                        // Once the server starts, lose both directions. In
                        // particular old Initial retransmissions cannot re-enter
                        // admission after the original attempt expires.
                        if started.is_some() {
                            proxy.send_to(&packet[..len], contact).await.unwrap();
                        }
                    }
                }
            });
            attempts.push(attacker.begin_connect(address).unwrap());
            timeout(Duration::from_secs(3), response)
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(helper.progress().admitted, 0);
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_secs(11)).await;
        assert_eq!(helper.progress().admitted, 0);
        assert!(!helper.progress().authorized);
        let connected = connect(decoded, "127.0.0.1:0".parse().unwrap(), &stop)
            .await
            .unwrap();
        assert_eq!(connected.route(), Route::Relay);
        let mut serving = JoinSet::new();
        serving.spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(socket.read_u8().await.unwrap());
            }
            assert!(header.starts_with(b"GET /recovered HTTP/1.1\r\n"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nrecovered",
                )
                .await
                .unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hostname = format!(
            "thepeer-{}.localhost",
            hex::encode(rand::random::<[u8; 16]>())
        );
        let authority = format!("{hostname}:{}", address.port());
        let browser_stop = stop.clone();
        serving.spawn(async move {
            connected
                .serve(listener, hostname, &browser_stop)
                .await
                .unwrap();
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(format!("http://{address}/recovered"))
            .header("Host", authority)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("content-security-policy"));
        assert_eq!(response.text().await.unwrap(), "recovered");
        stop.cancel();
        while let Some(done) = serving.join_next().await {
            done.unwrap();
        }
        helper.shutdown().await.unwrap();
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        for attempt in attempts {
            let _ = attempt.abort();
        }
        network_stop.cancel();
        while let Some(done) = forwarding.join_next().await {
            done.unwrap();
        }
        timeout(Duration::from_secs(8), attacker.shutdown())
            .await
            .unwrap();
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded virtual expiry and real browser recovery");
}

// Real packet loss only: no helper internals, synthetic TLS results or grants.
// Each wave alternates ordinary and virtual ingress into one shared budget.
struct MixedWave {
    attempts: Vec<Connecting>,
    peers: Vec<Connection>,
    replies: Vec<Arc<AtomicUsize>>,
    forwarding: JoinSet<()>,
    stop: CancellationToken,
}
impl MixedWave {
    async fn fill(client: &P2pEndpoint, contacts: [SocketAddr; 2]) -> Self {
        let mut wave = Self {
            attempts: Vec::new(),
            peers: Vec::new(),
            replies: Vec::new(),
            forwarding: JoinSet::new(),
            stop: CancellationToken::new(),
        };
        // Four active + four queued, then an overflow probe on EACH ingress.
        for slot in 0..10 {
            let contact = contacts[slot % 2];
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let address = proxy.local_addr().unwrap();
            let client_addr = client.local_addr().unwrap();
            let replies = Arc::new(AtomicUsize::new(0));
            let count = replies.clone();
            let done = wave.stop.clone();
            let (sent, arrived) = oneshot::channel();
            wave.forwarding.spawn(async move {
                let mut sent = Some(sent);
                let mut responded = false;
                let mut packet = vec![0; 65536];
                loop {
                    let (len, source) = tokio::select! {
                        _ = done.cancelled() => break,
                        received = proxy.recv_from(&mut packet) => received.unwrap(),
                    };
                    if source == contact {
                        responded = true;
                        count.fetch_add(1, Ordering::SeqCst);
                        proxy.send_to(&packet[..len], client_addr).await.unwrap();
                    } else {
                        assert_eq!(source, client_addr);
                        // After the first reply, lose client traffic (including
                        // Finished and retransmissions). Server TLS stays pending
                        // even if the client has received enough to complete TLS.
                        if !responded || slot >= 8 {
                            proxy.send_to(&packet[..len], contact).await.unwrap();
                            if let Some(sent) = sent.take() {
                                let _ = sent.send(());
                            }
                        }
                    }
                }
            });
            let attempt = client.begin_connect(address).unwrap();
            timeout(Duration::from_secs(3), arrived)
                .await
                .unwrap()
                .unwrap();
            if slot < 4 {
                timeout(Duration::from_secs(3), async {
                    while replies.load(Ordering::SeqCst) == 0 {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .expect("mixed active handshakes must start");
            } else if slot < 8 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                assert_eq!(
                    replies.load(Ordering::SeqCst),
                    0,
                    "ordinary and virtual ingress must share the active budget"
                );
            } else {
                let result = timeout(Duration::from_secs(3), attempt)
                    .await
                    .expect("both ingress kinds must refuse overflow");
                let refused = matches!(result,
                    Err(saorsa_transport::ConnectionError::ConnectionClosed(ref error))
                    if error.error_code == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED);
                assert!(refused, "shared overflow must receive CONNECTION_REFUSED");
                continue;
            }
            wave.attempts.push(attempt);
            wave.replies.push(replies);
        }
        wave
    }

    async fn expire(mut self) {
        // Observe queued work responding after the first batch expires,
        // then require actual client TLS success, not a late refusal.
        timeout(Duration::from_secs(13), async {
            while self.replies[4..]
                .iter()
                .any(|count| count.load(Ordering::SeqCst) == 0)
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("queued mixed handshakes must eventually start");
        for attempt in self.attempts.drain(4..) {
            let peer = timeout(Duration::from_secs(3), attempt)
                .await
                .expect("promoted queued client must finish real TLS")
                .expect("queued work must not be discarded with a late refusal");
            self.peers.push(peer);
        }
        // Their active deadline begins on promotion, not on arrival in the queue.
        tokio::time::sleep(Duration::from_secs(11)).await;
        self.finish().await;
    }

    async fn finish(mut self) {
        for peer in self.peers {
            peer.close(0u32.into(), b"fixture complete");
        }
        for attempt in self.attempts {
            let _ = attempt.abort();
        }
        self.stop.cancel();
        while let Some(done) = self.forwarding.join_next().await {
            done.unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_mixed_ingress_waves_remain_bounded_and_drop_releases_owner() {
    timeout(Duration::from_secs(90), async {
        let relay = node(true).await;
        let attacker = node(false).await;
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bind = reservation.local_addr().unwrap();
        drop(reservation);
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let parent = CancellationToken::new();
        let (helper, offer) = Helper::prepare_with_relay(
            HelperConfig {
                bind,
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: None,
            },
            relay.local_addr().unwrap(),
            &parent,
        )
        .await
        .unwrap();
        let text = offer.encode().unwrap();
        let decoded = Offer::decode(&text).unwrap();
        let mut contacts = [
            decoded.direct_candidate(),
            decoded.relay_candidate().unwrap(),
        ];
        drop((offer, text, decoded));
        for _ in 0..2 {
            let wave = MixedWave::fill(&attacker, contacts).await;
            assert_eq!(helper.progress().admitted, 0);
            assert!(!helper.progress().authorized);
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
            wave.expire().await;
            contacts.reverse();
        }
        // A third full wave proves capacity was recovered repeatedly. Abandon
        // the same helper with both its active and queued work still occupied.
        let wave = MixedWave::fill(&attacker, contacts).await;
        assert_eq!(helper.progress().admitted, 0);
        assert!(!helper.progress().authorized);
        drop(helper);
        let _released = timeout(Duration::from_secs(18), async {
            loop {
                match UdpSocket::bind(bind).await {
                    Ok(socket) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                        tokio::time::sleep(Duration::from_millis(20)).await
                    }
                    Err(error) => panic!("unexpected rebind failure: {error}"),
                }
            }
        })
        .await
        .expect("drop must release a repeatedly saturated helper bind");
        assert!(
            !parent.is_cancelled(),
            "helper drop must not cancel its parent"
        );
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        wave.finish().await;
        assert!(relay.is_running());
        timeout(Duration::from_secs(8), attacker.shutdown())
            .await
            .unwrap();
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded repeated mixed-ingress/drop fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_mixed_ingress_waves_preserve_offer_for_real_browser_http_and_stop() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    timeout(Duration::from_secs(90), async {
        let relay = node(true).await;
        let attacker = node(false).await;
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stop = CancellationToken::new();
        let (helper, offer) = Helper::prepare_with_relay(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: None,
            },
            relay.local_addr().unwrap(),
            &stop,
        )
        .await
        .unwrap();
        let text = offer.encode().unwrap();
        let decoded = Offer::decode(&text).unwrap();
        drop((offer, text));
        let bind = decoded.direct_candidate();
        let mut contacts = [bind, decoded.relay_candidate().unwrap()];
        for _ in 0..2 {
            let wave = MixedWave::fill(&attacker, contacts).await;
            assert_eq!(helper.progress().admitted, 0);
            assert!(!helper.progress().authorized);
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
            wave.expire().await;
            contacts.reverse();
        }
        assert_eq!(helper.progress().admitted, 0);
        assert!(!helper.progress().authorized);
        assert!(!stop.is_cancelled());
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        // The original offer remains usable; neither a helper restart nor a
        // replacement grant nor an automatic phone retry is involved.
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone_bind = reservation.local_addr().unwrap();
        drop(reservation);
        let connected = connect(decoded, phone_bind, &stop).await.unwrap();
        assert_eq!(connected.route(), Route::Relay);
        let mut serving = JoinSet::new();
        serving.spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(socket.read_u8().await.unwrap());
            }
            assert!(header.starts_with(b"GET /recovered HTTP/1.1\r\n"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nrecovered",
                )
                .await
                .unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hostname = format!(
            "thepeer-{}.localhost",
            hex::encode(rand::random::<[u8; 16]>())
        );
        let authority = format!("{hostname}:{}", address.port());
        let browser_stop = stop.clone();
        serving.spawn(async move {
            connected
                .serve(listener, hostname, &browser_stop)
                .await
                .unwrap();
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(format!("http://{address}/recovered"))
            .header("Host", authority)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("content-security-policy"));
        assert_eq!(response.text().await.unwrap(), "recovered");
        assert!(helper.progress().authorized);
        stop.cancel();
        while let Some(done) = serving.join_next().await {
            done.unwrap();
        }
        helper.shutdown().await.unwrap();
        let _released = UdpSocket::bind(bind).await.unwrap();
        let _phone_released = UdpSocket::bind(phone_bind).await.unwrap();
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        assert!(relay.is_running());
        timeout(Duration::from_secs(8), attacker.shutdown())
            .await
            .unwrap();
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded repeated mixed-ingress/browser recovery fixture");
}
