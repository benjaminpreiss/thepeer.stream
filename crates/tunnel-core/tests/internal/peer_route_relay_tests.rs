use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_and_coordinated_silence_reach_the_single_carrier_relay_for_http_and_stop() {
    relay_http(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_direct_relay_observation_and_relay_http_stop() {
    relay_http(true).await;
}

async fn relay_http(production: bool) {
    timeout(Duration::from_secs(50), async {
        // Only this synthetic loopback participant serves relay traffic. The runner
        // reuses the existing loopback-only allocation fixture configuration.
        let relay = P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: true,
                    advertise_external_addresses: false,
                    upnp: UpnpConfig::disabled(),
                    ..NatConfig::default()
                })
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, mut offer) = Helper::prepare_with_relay(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: relay.local_addr(),
            },
            relay.local_addr().unwrap(),
            &stop,
        )
        .await
        .unwrap();
        let relay_contact = offer.relay_candidate().unwrap();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.set_direct_candidate(quiet.local_addr().unwrap());
        let encoded = offer.encode().unwrap();
        let decoded = Offer::decode(&encoded).unwrap();
        drop((offer, encoded));
        let reserved = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone = reserved.local_addr().unwrap();
        drop(reserved);
        let connected = if production {
            super::connect(decoded, phone, &stop).await
        } else {
            connect(decoded, phone, &stop).await
        }
        .unwrap();
        assert_eq!(connected.route(), Route::Relay);
        if production {
            // Loopback observations are NOT public-WAN evidence. With a relay
            // available, the private candidates are ineligible. No punching.
            assert!(quiet.try_recv_from(&mut [0; 2048]).is_err());
        }
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(socket.read_u8().await.unwrap());
            }
            assert!(header.starts_with(b"GET /relay HTTP/1.1\r\n"));
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nrelayed",
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
            .get(format!("http://{address}/relay"))
            .header("Host", authority)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("content-security-policy"));
        assert_eq!(response.text().await.unwrap(), "relayed");
        stop.cancel();
        while let Some(done) = tasks.join_next().await {
            done.unwrap();
        }
        helper.shutdown().await.unwrap();
        assert_relay_contact_withdrawn(relay_contact).await;
        assert!(relay.is_running());
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        let _released = UdpSocket::bind(phone).await.unwrap();
    })
    .await
    .expect("bounded relay HTTP fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unauthorized_virtual_winner_cannot_reach_target_or_survive_stop_and_drop() {
    async fn node(serves_relay: bool) -> P2pEndpoint {
        P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: serves_relay,
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
    timeout(Duration::from_secs(55), async {
        for mode in [
            "idle-stop",
            "partial-stop",
            "idle-drop",
            "partial-drop",
            "unauthorized",
        ] {
            let relay = node(true).await;
            let attacker = node(false).await;
            let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let bind = reservation.local_addr().unwrap();
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let stop = CancellationToken::new();
            drop(reservation);
            let (mut helper, offer) = Helper::prepare_with_relay(
                HelperConfig {
                    bind,
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
            let encoded = offer.encode().unwrap();
            let decoded = Offer::decode(&encoded).unwrap();
            let contact = decoded.relay_candidate().unwrap();
            drop((offer, encoded, decoded));
            // The attacker gets only a contact, never the helper grant. This is
            // real TLS to the allocated virtual listener, not a mocked admission.
            attacker.connect(contact).await.unwrap();
            let peer = attacker
                .get_quic_connection(&contact)
                .await
                .unwrap()
                .unwrap();
            timeout(Duration::from_secs(3), async {
                while helper.progress().admitted == 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(helper.progress().admitted, 1);
            assert!(!helper.progress().authorized);
            let mut stream = if mode.starts_with("partial") || mode == "unauthorized" {
                Some(peer.open_bi().await.unwrap())
            } else {
                None
            };
            if let Some((send, recv)) = &mut stream {
                if mode == "unauthorized" {
                    crate::http::write_frame(
                        send,
                        br#"{"v":1,"type":"request","method":"GET","path":"/"}"#,
                    )
                    .await
                    .unwrap();
                    send.finish().unwrap();
                    let reply = timeout(Duration::from_secs(8), recv.read(&mut [0; 1]))
                        .await
                        .expect("unauthorized stream must close");
                    assert!(
                        matches!(reply, Ok(None) | Err(_)),
                        "unauthorized client must not receive an application acknowledgment"
                    );
                } else {
                    // Declare a record, then stall mid-body without FIN/reset.
                    let sent = peer.stats().frame_tx.stream;
                    send.write_all(&100u32.to_be_bytes()).await.unwrap();
                    send.write_all(b"{").await.unwrap();
                    timeout(Duration::from_secs(3), async {
                        while peer.stats().frame_tx.stream == sent {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .expect("partial record must really leave the client");
                    assert!(
                        timeout(Duration::from_millis(100), recv.read(&mut [0; 1]))
                            .await
                            .is_err(),
                        "no authorization acknowledgment may be sent"
                    );
                }
            }
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
            assert!(!helper.progress().authorized);
            if mode == "unauthorized" {
                assert_eq!(
                    timeout(Duration::from_secs(18), helper.wait())
                        .await
                        .expect("unauthorized session must finish")
                        .unwrap_err()
                        .0,
                    "application-auth-denied"
                );
                assert!(!helper.progress().authorized);
                helper.shutdown().await.unwrap();
            } else if mode.ends_with("drop") {
                drop(helper);
            } else {
                stop.cancel();
                timeout(Duration::from_secs(18), helper.wait())
                    .await
                    .expect("Stop must not wait for the authorization deadline")
                    .unwrap();
                helper.shutdown().await.unwrap();
            }
            timeout(Duration::from_secs(8), peer.closed())
                .await
                .unwrap();
            let _released = timeout(Duration::from_secs(10), async {
                loop {
                    if let Ok(socket) = UdpSocket::bind(bind).await {
                        break socket;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("helper UDP must be released, including dropped ownership");
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
            assert_eq!(stop.is_cancelled(), mode.ends_with("stop"));
            assert_relay_contact_withdrawn(contact).await;
            assert!(relay.is_running());
            drop(stream);
            timeout(Duration::from_secs(8), attacker.shutdown())
                .await
                .unwrap();
            timeout(Duration::from_secs(8), relay.shutdown())
                .await
                .unwrap();
        }
    })
    .await
    .expect("bounded hostile virtual-winner fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn selected_cross_family_observer_or_relay_is_terminal() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    timeout(Duration::from_secs(45), async {
        for coordinated in [true, false] {
            let participant = P2pEndpoint::new(
                P2pConfig::builder()
                    .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                    .nat(NatConfig {
                        allow_loopback: true,
                        enable_relay_service: true,
                        advertise_external_addresses: false,
                        upnp: UpnpConfig::disabled(),
                        ..NatConfig::default()
                    })
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();
            let participant_addr = participant.local_addr().unwrap();
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let stop = CancellationToken::new();
            let (helper, mut offer) = Helper::prepare_with_relay(
                HelperConfig {
                    bind: "127.0.0.1:0".parse().unwrap(),
                    target: format!("localhost:{}", target.local_addr().unwrap().port())
                        .parse()
                        .unwrap(),
                    observer: coordinated.then_some(participant_addr),
                },
                participant_addr,
                &stop,
            )
            .await
            .unwrap();
            let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            offer.set_direct_candidate(quiet.local_addr().unwrap());
            if coordinated {
                offer.observer = Some("[::1]:9".parse().unwrap());
            }
            let mut encoded = offer.encode().unwrap();
            if !coordinated {
                // Public wire seam: change only the relay address record.
                // Keep the real helper pin/grant opaque and unchanged.
                let mut packet = Zeroizing::new(URL_SAFE_NO_PAD.decode(&encoded[5..]).unwrap());
                assert_eq!(packet[1], 14, "relay, bound and observer flags");
                assert_eq!(packet[66], 4, "IPv4 Direct record");
                assert_eq!(packet[73], 4, "IPv4 relay address record");
                let port = offer.relay_candidate().unwrap().port();
                let mut replacement = vec![6];
                replacement.extend_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
                replacement.extend_from_slice(&port.to_be_bytes());
                // Preserve the bound and observer hints after this record.
                packet.splice(73..80, replacement);
                encoded = Zeroizing::new("wpa2:".to_owned());
                URL_SAFE_NO_PAD.encode_string(&*packet, &mut encoded);
            }
            let decoded = Offer::decode(&encoded).unwrap();
            drop((offer, encoded));
            let reserve = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let phone = reserve.local_addr().unwrap();
            drop(reserve);
            let result = connect(decoded, phone, &stop).await;
            let stage = result.as_ref().err().map(|error| error.0);
            drop(result);
            let shutdown = helper.shutdown().await;
            timeout(Duration::from_secs(8), participant.shutdown())
                .await
                .unwrap();
            assert_eq!(stage, Some("route-family"));
            shutdown.unwrap();
            assert!(
                quiet.try_recv_from(&mut [0; 2048]).is_err(),
                "private Direct is ineligible without a shared public observation"
            );
            let _released = UdpSocket::bind(phone).await.unwrap();
            assert!(!stop.is_cancelled());
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
        }
    })
    .await
    .expect("bounded selected-family fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn malformed_observer_is_terminal_before_a_healthy_direct_route() {
    timeout(Duration::from_secs(20), async {
        let relay = P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: true,
                    advertise_external_addresses: false,
                    upnp: UpnpConfig::disabled(),
                    ..NatConfig::default()
                })
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, mut offer) = Helper::prepare_with_relay(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target: format!("localhost:{}", target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: relay.local_addr(),
            },
            relay.local_addr().unwrap(),
            &stop,
        )
        .await
        .unwrap();
        offer.observer = Some("127.0.0.1:0".parse().unwrap());
        let result = connect(offer, "127.0.0.1:0".parse().unwrap(), &stop).await;
        assert!(matches!(result, Err(Failure("route-input"))));
        helper.shutdown().await.unwrap();
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded malformed offer fixture");
}

async fn assert_relay_contact_withdrawn(address: SocketAddr) {
    // ADR-011 deliberately retains the participant's bound port after closure.
    // Observe withdrawn peer reachability, not a promise of remote FD reclamation.
    let probe = P2pEndpoint::new(
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
    let attempt = probe.begin_connect(address).unwrap();
    let connected = match timeout(Duration::from_secs(2), attempt).await {
        Ok(Ok(peer)) => {
            peer.close(0u32.into(), b"unexpected retained relay peer");
            true
        }
        _ => false,
    };
    timeout(Duration::from_secs(8), probe.shutdown())
        .await
        .unwrap();
    assert!(
        !connected,
        "withdrawn relay contact must not complete peer TLS"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_a_prepared_helper_withdraws_the_relay_contact_without_target_access() {
    timeout(Duration::from_secs(20), async {
        let relay = P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: true,
                    advertise_external_addresses: false,
                    upnp: UpnpConfig::disabled(),
                    ..NatConfig::default()
                })
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
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
        let contact = offer.relay_candidate().unwrap();
        assert!(UdpSocket::bind(contact).await.is_err());
        drop(offer);
        helper.shutdown().await.unwrap();
        assert_relay_contact_withdrawn(contact).await;
        assert!(relay.is_running());
        assert!(!stop.is_cancelled());
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded unused allocation fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_winner_withdraws_unused_relay_but_keeps_browser_http_alive() {
    timeout(Duration::from_secs(25), async {
        let relay = P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: true,
                    advertise_external_addresses: false,
                    upnp: UpnpConfig::disabled(),
                    ..NatConfig::default()
                })
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
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
        let contact = offer.relay_candidate().unwrap();
        assert!(UdpSocket::bind(contact).await.is_err());
        // Advertise local-only Direct while retaining the helper's real relay
        // reservation, so withdrawal is exercised without inventing a WAN hint.
        let mut offer = offer;
        offer.relay = None;
        offer.observer = None;
        let connected = connect(offer, "127.0.0.1:0".parse().unwrap(), &stop)
            .await
            .unwrap();
        assert_eq!(connected.route(), Route::Direct);
        assert_relay_contact_withdrawn(contact).await;
        assert!(relay.is_running());
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(socket.read_u8().await.unwrap());
            }
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ndirect",
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
            .get(format!("http://{browser}/"))
            .header("Host", authority)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().await.unwrap(), "direct");
        stop.cancel();
        while let Some(done) = tasks.join_next().await {
            done.unwrap();
        }
        helper.shutdown().await.unwrap();
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded direct winner fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_direct_pin_is_terminal_without_authorizing_either_target() {
    timeout(Duration::from_secs(20), async {
        let relay = P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: true,
                    advertise_external_addresses: false,
                    upnp: UpnpConfig::disabled(),
                    ..NatConfig::default()
                })
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        let stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let wrong_target = TcpListener::bind("127.0.0.1:0").await.unwrap();
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
        let (wrong_helper, wrong_offer) = Helper::prepare(
            HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                target: format!("localhost:{}", wrong_target.local_addr().unwrap().port())
                    .parse()
                    .unwrap(),
                observer: None,
            },
            &stop,
        )
        .await
        .unwrap();
        // Local-only offer makes Direct eligible; pin errors remain terminal,
        // not route-unavailable. The helper still owns a real relay reservation.
        offer.relay = None;
        offer.observer = None;
        offer.set_direct_candidate(wrong_offer.direct_candidate());
        drop(wrong_offer);
        let result = connect(offer, "127.0.0.1:0".parse().unwrap(), &stop).await;
        assert!(matches!(result, Err(Failure("pin-mismatch"))));
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        assert!(
            timeout(Duration::from_millis(100), wrong_target.accept())
                .await
                .is_err()
        );
        helper.shutdown().await.unwrap();
        let _ = wrong_helper.shutdown().await;
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded relay terminal-pin fixture");
}
