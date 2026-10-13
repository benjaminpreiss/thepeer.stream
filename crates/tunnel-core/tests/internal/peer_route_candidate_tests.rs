use super::{Helper, HelperConfig, Offer, Route, connect};
use saorsa_transport::{NatConfig, P2pConfig, P2pEndpoint, upnp::UpnpConfig};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
    task::JoinSet,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

// Fault injector for QUIC v1 datagrams emitted by real fixture endpoints. Pass
// Initial/Handshake packets but remove trailing/coalesced 1-RTT packets, so TLS
// can complete while encrypted OBSERVED_ADDRESS frames cannot reach the helper.
fn handshake_prefix(packet: &[u8]) -> usize {
    fn varint(packet: &[u8], offset: &mut usize) -> usize {
        let size = 1 << (packet[*offset] >> 6);
        let mut value = (packet[*offset] & 0x3f) as usize;
        for byte in &packet[*offset + 1..*offset + size] {
            value = value * 256 + *byte as usize;
        }
        *offset += size;
        value
    }
    let mut offset = 0;
    while offset < packet.len() && packet[offset] & 0x80 != 0 {
        let kind = (packet[offset] >> 4) & 3;
        assert_eq!(
            &packet[offset + 1..offset + 5],
            &[0, 0, 0, 1],
            "fixture requires QUIC v1"
        );
        assert!(
            kind == 0 || kind == 2,
            "fixture expects Initial/Handshake only"
        );
        offset += 5;
        offset += 1 + packet[offset] as usize;
        offset += 1 + packet[offset] as usize;
        if kind == 0 {
            let token = varint(packet, &mut offset);
            offset += token;
        }
        let payload = varint(packet, &mut offset);
        offset += payload;
        assert!(offset <= packet.len());
    }
    offset
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn explicit_phone_family_mismatch_is_terminal_before_transport() {
    timeout(Duration::from_secs(25), async {
        for (computer, phone) in [("127.0.0.1:0", "[::1]:0"), ("[::1]:0", "127.0.0.1:0")] {
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let stop = CancellationToken::new();
            let (helper, offer) = Helper::prepare(
                HelperConfig {
                    bind: computer.parse().unwrap(),
                    target: format!("localhost:{}", target.local_addr().unwrap().port())
                        .parse()
                        .unwrap(),
                    observer: None,
                },
                &stop,
            )
            .await
            .unwrap();
            let text = offer.encode().unwrap();
            let decoded = Offer::decode(&text).unwrap();
            drop((offer, text));
            let reservation = UdpSocket::bind(phone).await.unwrap();
            let phone = reservation.local_addr().unwrap();
            drop(reservation);
            let result = connect(decoded, phone, &stop).await;
            let stage = result.as_ref().err().map(|error| error.0);
            drop(result);
            let shutdown = helper.shutdown().await;
            let _released = UdpSocket::bind(phone).await.unwrap();
            assert_eq!(
                stage,
                Some("route-family"),
                "local family mismatch is not remote silence"
            );
            shutdown.unwrap();
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
        }
    })
    .await
    .expect("bounded explicit-family fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn preparation_rejects_unrepresentable_bind_and_invalid_coordinator() {
    timeout(Duration::from_secs(20), async {
        let reserved = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let occupied = reserved.local_addr().unwrap();
        // Occupy the bind in invalid-contact cases: an implementation that skips
        // preflight fails binding instead of sending to a non-loopback contact.
        let invalid = [
            "0.0.0.0:9",
            "[::]:9",
            "224.0.0.1:9",
            "255.255.255.255:9",
            "[ff02::1]:9",
            "[fe80::1]:9",
            "[::ffff:127.0.0.1]:9",
        ]
        .map(|address| address.parse::<SocketAddr>().unwrap());
        let mut cases = vec![
            ("[::ffff:127.0.0.1]:0".parse().unwrap(), None),
            (occupied, Some("127.0.0.1:0".parse().unwrap())),
            (occupied, Some("[::1]:0".parse().unwrap())),
            (
                SocketAddr::new("0.0.0.0".parse().unwrap(), occupied.port()),
                None,
            ),
            ("[::]:0".parse().unwrap(), None),
        ];
        cases.extend(invalid.into_iter().map(|address| (occupied, Some(address))));
        for (flow, scope) in [(1, 0), (0, 1)] {
            let address = SocketAddr::V6(std::net::SocketAddrV6::new(
                std::net::Ipv6Addr::LOCALHOST,
                9,
                flow,
                scope,
            ));
            cases.push((address, None));
            cases.push((occupied, Some(address)));
        }
        for (bind, coordinator) in cases {
            let result = Helper::prepare(
                HelperConfig {
                    bind,
                    target: format!("localhost:{}", target.local_addr().unwrap().port())
                        .parse()
                        .unwrap(),
                    observer: coordinator,
                },
                &CancellationToken::new(),
            )
            .await;
            let stage = result.as_ref().err().map(|error| error.0);
            if let Ok((helper, _)) = result {
                helper.shutdown().await.unwrap();
            }
            assert_eq!(
                stage,
                Some("route-input"),
                "reject input before endpoint creation"
            );
        }
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
    })
    .await
    .expect("bounded invalid configuration fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unusable_participant_is_rejected_before_coordinator_traffic() {
    for (relay, wrong_coordinator, expected) in [
        ("127.0.0.1:0", false, "route-input"),
        ("[::1]:9", false, "route-family"),
        ("127.0.0.1:9", true, "route-family"),
    ] {
        timeout(Duration::from_secs(15), async {
            let coordinator = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let bind = reservation.local_addr().unwrap();
            drop(reservation);
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let stop = CancellationToken::new();
            let preparing = Helper::prepare_with_relay(
                HelperConfig {
                    bind,
                    target: format!("localhost:{}", target.local_addr().unwrap().port())
                        .parse()
                        .unwrap(),
                    observer: Some(if wrong_coordinator {
                        "[::1]:9".parse().unwrap()
                    } else {
                        coordinator.local_addr().unwrap()
                    }),
                },
                relay.parse().unwrap(),
                &stop,
            );
            tokio::pin!(preparing);
            let mut packet = [0; 65536];
            let result = tokio::select! {
                result = &mut preparing => Some(result),
                received = coordinator.recv_from(&mut packet) => { received.unwrap(); None },
            };
            // Drain a broken implementation before asserting; the red test must not
            // strand the private runtime or a capability-bearing successful helper.
            let sent = result.is_none();
            let result = match result {
                Some(result) => result,
                None => {
                    stop.cancel();
                    preparing.await
                }
            };
            let stage = result.as_ref().err().map(|error| error.0);
            if let Ok((helper, _)) = result {
                helper.shutdown().await.unwrap();
            }
            let _released = UdpSocket::bind(bind).await.unwrap();
            assert!(
                !sent,
                "invalid configuration started coordinator networking"
            );
            assert_eq!(stage, Some(expected));
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
        })
        .await
        .expect("bounded preparation preflight fixture");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn withheld_observation_after_real_tls_is_terminal_and_cancellable() {
    timeout(Duration::from_secs(35), async {
        for mode in [
            "pass",
            "silence",
            "foreign",
            "stop",
            "relay-silence",
            "relay-stop",
            "closed",
            "relay-closed",
        ] {
            let seed = P2pEndpoint::new(
                P2pConfig::builder()
                    .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                    .nat(NatConfig {
                        allow_loopback: true,
                        enable_relay_service: false,
                        advertise_external_addresses: false,
                        upnp: UpnpConfig::disabled(),
                        ..NatConfig::default()
                    })
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();
            let seed_addr = seed.local_addr().unwrap();
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let coordinator = proxy.local_addr().unwrap();
            let reserve = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let bind = reserve.local_addr().unwrap();
            drop(reserve);
            let network_stop = CancellationToken::new();
            let done = network_stop.clone();
            let (dropped, observed_drop) = tokio::sync::oneshot::channel();
            let forward = tokio::spawn(async move {
                let mut dropped = Some(dropped);
                let mut packet = vec![0; 65536];
                loop {
                    let (len, source) = tokio::select! {
                        _ = done.cancelled() => break,
                        received = proxy.recv_from(&mut packet) => received.unwrap(),
                    };
                    if source == seed_addr {
                        let prefix = handshake_prefix(&packet[..len]);
                        // The close cases use a deliberately large, authenticated
                        // close reason. Keep small observation-bearing packets
                        // lost, but let that close reach the helper. No ciphertext
                        // is decrypted, rewritten, or synthesized by this proxy.
                        let forwarded = if mode == "pass"
                            || (mode.ends_with("closed") && len - prefix >= 512)
                        {
                            len
                        } else {
                            prefix
                        };
                        if forwarded < len {
                            if let Some(sender) = dropped.take() {
                                let _ = sender.send(());
                            }
                        }
                        if forwarded > 0 {
                            proxy.send_to(&packet[..forwarded], bind).await.unwrap();
                        }
                    } else {
                        assert_eq!(source, bind);
                        proxy.send_to(&packet[..len], seed_addr).await.unwrap();
                    }
                }
            });
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let config = HelperConfig {
                bind,
                target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(),
                observer: (!mode.starts_with("relay-")).then_some(coordinator),
            };
            let stop = CancellationToken::new();
            let preparing_stop = stop.clone();
            let preparing = tokio::spawn(async move {
                if mode.starts_with("relay-") {
                    Helper::prepare_with_relay(config, coordinator, &preparing_stop).await
                } else {
                    Helper::prepare(config, &preparing_stop).await
                }
            });
            let (_, control) = seed.accept_with_connection().await.unwrap();
            if mode != "pass" {
                observed_drop.await.unwrap();
            }
            let foreign = if mode == "foreign" {
                let node = P2pEndpoint::new(
                    P2pConfig::builder()
                        .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                        .nat(NatConfig {
                            allow_loopback: true,
                            enable_relay_service: false,
                            advertise_external_addresses: false,
                            upnp: UpnpConfig::disabled(),
                            ..NatConfig::default()
                        })
                        .build()
                        .unwrap(),
                )
                .await
                .unwrap();
                // Preparation now rejects unsolicited ingress before TLS. The
                // independent-peer observation positive control is retained at
                // the public transport seam in peer_handshake_tests instead.
                let dial = node.begin_connect(bind).unwrap();
                let result = timeout(Duration::from_secs(2), dial).await.unwrap();
                let refused = matches!(result,
                    Err(saorsa_transport::ConnectionError::ConnectionClosed(ref error))
                    if error.error_code == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED);
                assert!(refused, "foreign preparation ingress must be explicitly refused");
                Some(node)
            } else {
                None
            };
            if mode.ends_with("stop") {
                stop.cancel();
            }
            if mode.ends_with("closed") {
                control.close(0u32.into(), &[b'x'; 512]);
            }
            let result = preparing.await.unwrap();
            if mode == "pass" {
                let (helper, offer) = result.unwrap();
                assert_eq!(offer.direct_candidate(), coordinator);
                helper.shutdown().await.unwrap();
            } else {
                assert_eq!(
                    result.as_ref().err().map(|error| error.0),
                    Some(if mode.ends_with("stop") {
                        "peer-cancelled"
                    } else if mode.ends_with("closed") {
                        "route-observer-control"
                    } else {
                        "route-observation-timeout"
                    }),
                    "no offer may escape without the requested observation"
                );
            }
            timeout(Duration::from_secs(3), control.closed())
                .await
                .unwrap();
            let _released = UdpSocket::bind(bind).await.unwrap();
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
            network_stop.cancel();
            forward.await.unwrap();
            timeout(Duration::from_secs(8), seed.shutdown())
                .await
                .unwrap();
            if let Some(foreign) = foreign {
                timeout(Duration::from_secs(8), foreign.shutdown())
                    .await
                    .unwrap();
            }
        }
    })
    .await
    .expect("bounded observation packet-loss fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observed_mapping_is_preserved_separately_from_bound_and_relay_candidates() {
    for (bind, relay_only) in [
        ("127.0.0.1:0", false),
        ("[::1]:0", false),
        ("127.0.0.1:0", true),
    ] {
        timeout(Duration::from_secs(25), async {
            let seed = P2pEndpoint::new(
                P2pConfig::builder()
                    .bind_addr(bind.parse::<SocketAddr>().unwrap())
                    .nat(NatConfig {
                        allow_loopback: true,
                        enable_relay_service: relay_only,
                        advertise_external_addresses: false,
                        upnp: UpnpConfig::disabled(),
                        ..NatConfig::default()
                    })
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();
            let coordinator = seed.local_addr().unwrap();
            let reservation = UdpSocket::bind(bind).await.unwrap();
            let computer = reservation.local_addr().unwrap();
            drop(reservation);
            let front = UdpSocket::bind(bind).await.unwrap();
            let participant = front.local_addr().unwrap();
            let outside = UdpSocket::bind(bind).await.unwrap();
            let observed = outside.local_addr().unwrap();
            let incoming = UdpSocket::bind(bind).await.unwrap();
            let stop = CancellationToken::new();
            let translation_stop = stop.clone();
            let mut tasks = JoinSet::new();
            // Real packets only: coordinator sees a different source port from bind.
            // A separate inbound leg preserves the helper's control/application paths.
            tasks.spawn(async move {
                let mut phone = None;
                let (mut a, mut b, mut c) = (vec![0; 65536], vec![0; 65536], vec![0; 65536]);
                loop {
                    tokio::select! {
                        _ = translation_stop.cancelled() => break,
                        received = front.recv_from(&mut a) => {
                            let (len, source) = received.unwrap();
                            assert_eq!(source, computer);
                            outside.send_to(&a[..len], coordinator).await.unwrap();
                        }
                        received = outside.recv_from(&mut b) => {
                            let (len, source) = received.unwrap();
                            if source == coordinator {
                                front.send_to(&b[..len], computer).await.unwrap();
                            } else {
                                if let Some(previous) = phone { assert_eq!(previous, source); }
                                phone = Some(source);
                                incoming.send_to(&b[..len], computer).await.unwrap();
                            }
                        }
                        received = incoming.recv_from(&mut c) => {
                            let (len, source) = received.unwrap();
                            assert_eq!(source, computer);
                            outside.send_to(&c[..len], phone.unwrap()).await.unwrap();
                        }
                    }
                }
            });
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let config = HelperConfig {
                bind: computer,
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: (!relay_only).then_some(participant),
            };
            let (helper, mut offer) = if relay_only {
                Helper::prepare_with_relay(config, participant, &stop).await
            } else {
                Helper::prepare(config, &stop).await
            }
            .unwrap();
            if relay_only {
                assert_ne!(
                    offer.relay_candidate().unwrap(),
                    observed,
                    "allocation contact must not become the direct hint"
                );
            }
            // The source-translation proxy is helper-specific. Let the phone
            // observe against the real participant, not that one-client proxy.
            offer.observer = Some(coordinator);
            assert_ne!(observed, computer);
            assert_eq!(
                offer.direct_candidate(),
                observed,
                "offer must use the actual observed mapping, not local bind"
            );
            let encoded = offer.encode().unwrap();
            let decoded = Offer::decode(&encoded).unwrap();
            drop((offer, encoded));
            let connected = connect(decoded, bind.parse().unwrap(), &stop)
                .await
                .unwrap();
            assert_eq!(
                connected.route(),
                if relay_only {
                    Route::Relay
                } else {
                    Route::Direct
                }
            );
            tasks.spawn(async move {
                let (mut socket, _) = target.accept().await.unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    assert!(header.len() < 16384);
                    header.push(socket.read_u8().await.unwrap());
                }
                assert!(header.starts_with(b"GET /observed HTTP/1.1\r\n"));
                socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nobserved",
                )
                .await
                .unwrap();
            });
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let browser = listener.local_addr().unwrap();
            let hostname = format!(
                "thepeer-{}.localhost",
                hex::encode(rand::random::<[u8; 16]>())
            );
            let authority = format!("{hostname}:{}", browser.port());
            let browser_stop = stop.clone();
            tasks.spawn(async move {
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
                .get(format!("http://{browser}/observed"))
                .header("Host", authority)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert!(!response.headers().contains_key("content-security-policy"));
            assert_eq!(response.text().await.unwrap(), "observed");
            stop.cancel();
            while let Some(result) = tasks.join_next().await {
                result.unwrap();
            }
            helper.shutdown().await.unwrap();
            let _released = UdpSocket::bind(computer).await.unwrap();
            timeout(Duration::from_secs(8), seed.shutdown())
                .await
                .unwrap();
        })
        .await
        .expect("bounded source-translation candidate fixture");
    }
}
