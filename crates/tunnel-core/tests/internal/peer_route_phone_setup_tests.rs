//! Phone preparation at the public connect seam: real packet barriers, no private state.
use super::*;
use tokio::net::UdpSocket;

async fn node() -> P2pEndpoint {
    P2pEndpoint::new(
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
    .unwrap()
}

async fn blocked_setup_refuses_ingress(observed: bool, abandon: bool) {
    timeout(Duration::from_secs(45), async {
        let helper_reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let helper_bind = helper_reservation.local_addr().unwrap();
        let phone_reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone_bind = phone_reservation.local_addr().unwrap();
        drop((helper_reservation, phone_reservation));
        let participant = node().await;
        let participant_addr = participant.local_addr().unwrap();
        let attacker = node().await;
        let attacker_addr = attacker.local_addr().unwrap();
        let downstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let observer_addr = downstream.local_addr().unwrap();
        let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let (first_phone, phone_initial) = oneshot::channel();
        let network_stop = CancellationToken::new();
        let _network_guard = network_stop.clone().drop_guard();
        let mut forwarding = JoinSet::new();
        let done = network_stop.clone();
        forwarding.spawn(async move {
            let mut first_phone = Some(first_phone);
            let mut request = vec![0; 65536];
            let mut response = vec![0; 65536];
            loop {
                tokio::select! {
                    _ = done.cancelled() => break,
                    received = downstream.recv_from(&mut request) => {
                        let (len, source) = received.unwrap();
                        if source == helper_bind {
                            upstream.send_to(&request[..len], participant_addr).await.unwrap();
                        } else {
                            assert_eq!(source, phone_bind);
                            // Only the helper's real control traffic is forwarded.
                            // Phone Initial proves observer bootstrap has started.
                            if let Some(first) = first_phone.take() { let _ = first.send(()); }
                        }
                    },
                    received = upstream.recv_from(&mut response) => {
                        let (len, source) = received.unwrap();
                        assert_eq!(source, participant_addr);
                        downstream.send_to(&response[..len], helper_bind).await.unwrap();
                    },
                }
            }
        });
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let helper_stop = CancellationToken::new();
        let (helper, mut offer) = Helper::prepare(
            HelperConfig {
                bind: helper_bind,
                target_port: target.local_addr().unwrap().port(),
                observer: observed.then_some(observer_addr),
            },
            &helper_stop,
        )
        .await
        .unwrap();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.set_direct_candidate(quiet.local_addr().unwrap());
        let decoded = Offer::decode(&offer.encode().unwrap()).unwrap();
        drop(offer);
        let stop = CancellationToken::new();
        let _stop_guard = stop.clone().drop_guard();
        let connect_stop = stop.clone();
        let connecting =
            tokio::spawn(async move { connect(decoded, phone_bind, &connect_stop).await });
        if observed {
            timeout(Duration::from_secs(10), phone_initial)
                .await
                .unwrap()
                .unwrap();
        } else {
            let mut packet = vec![0; 65536];
            let (_, source) = timeout(Duration::from_secs(2), quiet.recv_from(&mut packet))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(source, phone_bind);
        }
        let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = proxy.local_addr().unwrap();
        let done = network_stop.clone();
        forwarding.spawn(async move {
            let mut responded = false;
            let mut packet = vec![0; 65536];
            loop {
                let (len, source) = tokio::select! {
                    _ = done.cancelled() => break,
                    received = proxy.recv_from(&mut packet) => received.unwrap(),
                };
                if source == phone_bind {
                    responded = true;
                    proxy.send_to(&packet[..len], attacker_addr).await.unwrap();
                } else {
                    assert_eq!(source, attacker_addr);
                    if !responded {
                        proxy.send_to(&packet[..len], phone_bind).await.unwrap();
                    }
                }
            }
        });
        let mut attempt = attacker.begin_connect(address).unwrap();
        let mut peer = None;
        let mut pending = None;
        let mut refused = false;
        match timeout(Duration::from_millis(700), &mut attempt).await {
            Ok(Ok(connection)) => peer = Some(connection),
            Ok(Err(saorsa_transport::ConnectionError::ConnectionClosed(error)))
                if error.error_code == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED =>
            {
                refused = true;
            }
            Ok(Err(error)) => panic!("unexpected ingress outcome: {error}"),
            Err(_) => pending = Some(attempt),
        }
        let still_preparing = !connecting.is_finished();
        let no_target = timeout(Duration::from_millis(100), target.accept())
            .await
            .is_err();
        if abandon {
            connecting.abort();
            let result = timeout(Duration::from_secs(18), connecting).await.unwrap();
            assert!(matches!(result, Err(error) if error.is_cancelled()));
            assert!(!stop.is_cancelled());
        } else {
            stop.cancel();
            let result = timeout(Duration::from_secs(18), connecting)
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(result, Err(Failure("peer-cancelled"))));
        }
        let _released = timeout(Duration::from_secs(18), async {
            loop {
                match UdpSocket::bind(phone_bind).await {
                    Ok(socket) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                        tokio::time::sleep(Duration::from_millis(20)).await
                    }
                    Err(error) => panic!("unexpected rebind failure: {error}"),
                }
            }
        })
        .await
        .expect("abandoned phone preparation releases UDP");
        assert_eq!(helper.progress().admitted, 0);
        assert!(!helper.progress().authorized);
        assert!(!helper_stop.is_cancelled());
        helper.shutdown().await.unwrap();
        let completed = peer.is_some();
        if let Some(peer) = peer {
            peer.close(0u32.into(), b"fixture complete");
        }
        if let Some(attempt) = pending {
            let _ = attempt.abort();
        }
        network_stop.cancel();
        while let Some(done) = forwarding.join_next().await {
            done.unwrap();
        }
        timeout(Duration::from_secs(8), participant.shutdown())
            .await
            .unwrap();
        timeout(Duration::from_secs(8), attacker.shutdown())
            .await
            .unwrap();
        assert!(
            still_preparing,
            "attack must run before route setup completes"
        );
        assert!(no_target);
        assert!(!completed, "phone setup must not start unsolicited TLS");
        assert!(refused, "dormant phone ingress must explicitly refuse");
    })
    .await
    .expect("bounded phone setup ingress fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_setup_refuses_unsolicited_tls_and_stops() {
    blocked_setup_refuses_ingress(false, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn observer_bootstrap_refuses_unsolicited_tls_and_stops() {
    blocked_setup_refuses_ingress(true, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn abandoning_observer_bootstrap_refuses_tls_and_releases_owner() {
    blocked_setup_refuses_ingress(true, true).await;
}
