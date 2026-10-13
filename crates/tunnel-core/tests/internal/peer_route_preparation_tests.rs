//! Preparation admission regressions, promoted from the bounded red diagnostic.
//! Five loopback attempts only. No grants, relay serving or public traffic.
use crate::peer_route::{Helper, HelperConfig};
use saorsa_transport::{NatConfig, P2pConfig, P2pEndpoint, upnp::UpnpConfig};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::{TcpListener, UdpSocket},
    task::JoinSet,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

async fn completed_client_handshakes(hold_bootstrap: bool, abandon: bool) -> usize {
    timeout(Duration::from_secs(45), async {
        let attacker = P2pEndpoint::new(
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
        let reserved = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bind = reserved.local_addr().unwrap();
        drop(reserved);
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        // A bound, silent UDP participant models total bootstrap packet loss.
        // Seeing its first real Initial proves helper endpoint creation occurred.
        let silent = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let stop = CancellationToken::new();
        let config = HelperConfig {
            bind,
            target_port: target.local_addr().unwrap().port(),
            observer: hold_bootstrap.then(|| silent.local_addr().unwrap()),
        };
        let mut ready = None;
        let mut preparing = None;
        if hold_bootstrap {
            let preparation_stop = stop.clone();
            preparing = Some(tokio::spawn(async move {
                Helper::prepare(config, &preparation_stop).await
            }));
            let mut packet = vec![0; 65536];
            let (_, source) = timeout(Duration::from_secs(3), silent.recv_from(&mut packet))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(source, bind);
        } else {
            let (helper, offer) = Helper::prepare(config, &stop).await.unwrap();
            drop(offer);
            ready = Some(helper);
        }
        let network_stop = CancellationToken::new();
        let mut forwarding = JoinSet::new();
        let mut pending = Vec::new();
        let mut peers = Vec::new();
        let mut refused = 0;
        for _ in 0..5 {
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let address = proxy.local_addr().unwrap();
            let client_addr = attacker.local_addr().unwrap();
            let done = network_stop.clone();
            forwarding.spawn(async move {
                let mut responded = false;
                let mut packet = vec![0; 65536];
                loop {
                    let (len, source) = tokio::select! {
                        _ = done.cancelled() => break,
                        received = proxy.recv_from(&mut packet) => received.unwrap(),
                    };
                    if source == bind {
                        responded = true;
                        proxy.send_to(&packet[..len], client_addr).await.unwrap();
                    } else {
                        assert_eq!(source, client_addr);
                        // Forward genuine server flights but withhold client
                        // Finished. Client-side TLS success proves more than
                        // receipt of an ACK/refusal; helper TLS stays unfinished.
                        if !responded {
                            proxy.send_to(&packet[..len], bind).await.unwrap();
                        }
                    }
                }
            });
            let mut attempt = attacker.begin_connect(address).unwrap();
            match timeout(Duration::from_millis(700), &mut attempt).await {
                Ok(Ok(peer)) => peers.push(peer),
                Ok(Err(saorsa_transport::ConnectionError::ConnectionClosed(error)))
                    if error.error_code
                        == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED =>
                {
                    refused += 1;
                }
                Ok(Err(error)) => panic!("unexpected admission failure: {error}"),
                Err(_) => pending.push(attempt),
            }
        }
        let completed = peers.len();
        assert!(
            peers.iter().all(|peer| peer.close_reason().is_none()),
            "successful client connections must still be live together"
        );
        let was_preparing = preparing.as_ref().is_some_and(|task| !task.is_finished());
        if let Some(helper) = &ready {
            assert_eq!(helper.progress().admitted, 0);
            assert!(!helper.progress().authorized);
        }
        // Always clean up before asserting the diagnostic bound. A red run
        // must not strand a private helper runtime or return/persist an offer.
        if !abandon {
            stop.cancel();
        }
        let mut stage = None;
        if abandon {
            let task = preparing.take().unwrap();
            task.abort();
            let result = timeout(Duration::from_secs(18), task).await.unwrap();
            assert!(matches!(result, Err(error) if error.is_cancelled()));
            assert!(
                !stop.is_cancelled(),
                "abandonment must not cancel the parent"
            );
        }
        if let Some(task) = preparing {
            let result = timeout(Duration::from_secs(18), task)
                .await
                .unwrap()
                .unwrap();
            stage = result.as_ref().err().map(|error| error.0);
            if let Ok((helper, offer)) = result {
                drop(offer);
                helper.shutdown().await.unwrap();
            }
        }
        if let Some(helper) = ready {
            helper.shutdown().await.unwrap();
        }
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
        .expect("preparation stop/drop must release UDP");
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        for peer in peers {
            peer.close(0u32.into(), b"diagnostic complete");
        }
        for attempt in pending {
            let _ = attempt.abort();
        }
        network_stop.cancel();
        while let Some(done) = forwarding.join_next().await {
            done.unwrap();
        }
        timeout(Duration::from_secs(8), attacker.shutdown())
            .await
            .unwrap();
        if hold_bootstrap {
            assert!(was_preparing, "probe must run before preparation completes");
            if !abandon {
                assert_eq!(stage, Some("peer-cancelled"), "Stop must return no offer");
            }
            assert_eq!(
                refused, 5,
                "preparation must refuse, not silently defer unsolicited ingress"
            );
        }
        completed
    })
    .await
    .expect("bounded five-attempt preparation diagnostic")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ready_helper_control_holds_fifth_unfinished_handshake() {
    assert_eq!(completed_client_handshakes(false, false).await, 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn preparation_refuses_unsolicited_tls_and_stop_releases_owner() {
    assert_eq!(completed_client_handshakes(true, false).await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn abandoning_preparation_refuses_unsolicited_tls_and_releases_owner() {
    assert_eq!(completed_client_handshakes(true, true).await, 0);
}
