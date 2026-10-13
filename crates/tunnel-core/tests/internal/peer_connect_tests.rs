use super::*;
use crate::http::{host::Counts, inspector::HttpInspector};
use saorsa_transport::{
    NatConfig, P2pConfig,
    crypto::{pqc::types::MlDsaPublicKey, raw_public_keys::fingerprint_public_key},
    upnp::UpnpConfig,
};
use std::time::Duration;
use tokio::{net::TcpListener, task::JoinSet, time::timeout};
use zeroize::Zeroizing;

async fn node() -> P2pEndpoint {
    P2pEndpoint::new(
        P2pConfig::builder()
            .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .max_connections(4)
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
fn pin(node: &P2pEndpoint) -> [u8; 32] {
    fingerprint_public_key(&MlDsaPublicKey::from_bytes(node.public_key_bytes()).unwrap())
}
async fn shutdown(nodes: &[&P2pEndpoint]) {
    for node in nodes {
        timeout(Duration::from_secs(8), node.shutdown())
            .await
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn established_attempt_authorizes_the_grant_and_session_drop_closes_the_peer() {
    let phone = node().await;
    let computer = node().await;
    let mut tasks = JoinSet::new();
    let result = timeout(Duration::from_secs(15), async {
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = target.local_addr().unwrap().port();
        let (inspector, grant) = HttpInspector::new(port).unwrap();
        let grant = Zeroizing::new(grant);
        let helper = computer.clone();
        tasks.spawn(async move {
            let (_, connection) = helper.accept_with_connection().await.unwrap();
            let mut counts = Counts::default();
            crate::application::serve_peer(connection, inspector, port, &mut counts).await?;
            assert!(counts.authorized);
            assert_eq!(counts.fetched, 0);
            Ok::<(), Failure>(())
        });
        let session = open(
            &phone,
            computer.local_addr().unwrap(),
            pin(&computer),
            &grant,
            &CancellationToken::new(),
        )
        .await?
        .ok_or(Failure("test-peer-unavailable"))?;
        drop(grant);
        drop(session);
        tasks.join_next().await.unwrap().unwrap()?;
        Ok::<(), Failure>(())
    })
    .await;
    tasks.shutdown().await;
    shutdown(&[&phone, &computer]).await;
    result
        .expect("bounded attempt fixture")
        .expect("authorized peer session");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_silent_udp_target_permits_fallback_within_the_response_deadline() {
    let phone = node().await;
    let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let result = timeout(
        Duration::from_secs(5),
        open(
            &phone,
            silent.local_addr().unwrap(),
            [0; 32],
            &grant,
            &CancellationToken::new(),
        ),
    )
    .await;
    shutdown(&[&phone]).await;
    assert!(
        matches!(result, Ok(Ok(None))),
        "only a confirmed silent attempt may permit fallback"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_peer_response_followed_by_a_stalled_tls_handshake_is_terminal() {
    let phone = node().await;
    let computer = node().await;
    let mut tasks = JoinSet::new();
    let relay = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = relay.local_addr().unwrap();
    let target = computer.local_addr().unwrap();
    // Network fault injection only: real QUIC/TLS on both sides. Forward just
    // the first server UDP datagram; it cannot contain the full ML-DSA handshake.
    tasks.spawn(async move {
        let mut buffer = [0u8; 65536];
        let mut client = None;
        let mut replied = false;
        loop {
            let (size, source) = relay.recv_from(&mut buffer).await.unwrap();
            if source == target {
                if !replied {
                    relay
                        .send_to(&buffer[..size], client.unwrap())
                        .await
                        .unwrap();
                    replied = true;
                }
            } else {
                client = Some(source);
                relay.send_to(&buffer[..size], target).await.unwrap();
            }
        }
    });
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let result = timeout(
        Duration::from_secs(15),
        open(
            &phone,
            address,
            pin(&computer),
            &grant,
            &CancellationToken::new(),
        ),
    )
    .await;
    tasks.shutdown().await;
    shutdown(&[&phone, &computer]).await;
    assert!(
        matches!(result, Ok(Err(Failure("peer-handshake-timeout")))),
        "post-response timeout must not become unavailable"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_cancels_a_pending_attempt_without_closing_a_shared_control_connection() {
    let phone = node().await;
    let coordinator = node().await;
    let seed = coordinator.local_addr().unwrap();
    phone.connect(seed).await.unwrap();
    let (_, receiver) = coordinator.accept_with_connection().await.unwrap();
    let sender = phone.get_quic_connection(&seed).await.unwrap().unwrap();
    let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let stop = CancellationToken::new();
    let result = timeout(Duration::from_secs(5), async {
        let pending = open(&phone, silent.local_addr().unwrap(), [0; 32], &grant, &stop);
        tokio::pin!(pending);
        let mut packet = [0; 2048];
        tokio::select! {
            _=&mut pending=>panic!("silent peer cannot complete setup"),
            received=silent.recv_from(&mut packet)=>{received.unwrap();}
        }
        stop.cancel();
        assert!(matches!(
            timeout(Duration::from_secs(1), pending).await,
            Ok(Err(Failure("peer-cancelled")))
        ));
        let (mut send, _) = sender.open_bi().await.unwrap();
        send.write_all(b"control alive").await.unwrap();
        send.finish().unwrap();
        let (_, mut recv) = receiver.accept_bi().await.unwrap();
        assert_eq!(recv.read_to_end(64).await.unwrap(), b"control alive");
    })
    .await;
    shutdown(&[&phone, &coordinator]).await;
    result.expect("bounded cancellation fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_correct_pin_with_a_forged_tls_signature_is_terminal_not_unavailable() {
    use saorsa_transport::crypto::raw_public_keys::generate_ml_dsa_keypair;
    // Positive control uses the identical server configuration. Only the signing
    // key mismatch changes: otherwise an unrelated TLS incompatibility could make
    // the negative assertion pass without actually exercising signature rejection.
    for forged in [false, true] {
        let phone = node().await;
        let (public, correct_secret) = generate_ml_dsa_keypair().unwrap();
        let secret = if forged {
            generate_ml_dsa_keypair().unwrap().1
        } else {
            correct_secret
        };
        let expected = fingerprint_public_key(&public);
        let server = P2pEndpoint::new(
            P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .max_connections(4)
                .nat(NatConfig {
                    allow_loopback: true,
                    enable_relay_service: false,
                    advertise_external_addresses: false,
                    upnp: UpnpConfig::disabled(),
                    ..NatConfig::default()
                })
                .keypair(public, secret)
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
        let mut tasks = JoinSet::new();
        let (inspector, grant) = HttpInspector::new(1234).unwrap();
        let grant = Zeroizing::new(grant);
        let result = timeout(Duration::from_secs(8), async {
            if !forged {
                let helper = server.clone();
                tasks.spawn(async move {
                    let (_, accepted) = helper.accept_with_connection().await.unwrap();
                    let mut counts = Counts::default();
                    crate::application::serve_peer(accepted, inspector, 1234, &mut counts)
                        .await
                        .unwrap();
                    assert!(counts.authorized);
                });
            }
            {
                let result = open(
                    &phone,
                    server.local_addr().unwrap(),
                    expected,
                    &grant,
                    &CancellationToken::new(),
                )
                .await;
                if forged {
                    assert!(
                        matches!(result, Err(Failure("peer-response" | "peer-handshake"))),
                        "bad TLS signature is terminal"
                    );
                } else {
                    drop(
                        result
                            .unwrap()
                            .expect("correct signer must authorize the session"),
                    );
                }
            }
            while let Some(done) = tasks.join_next().await {
                done.unwrap();
            }
        })
        .await;
        tasks.shutdown().await;
        shutdown(&[&phone, &server]).await;
        result.expect("bounded signature fixture with positive control");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_grants_fail_before_any_network_attempt() {
    let phone = node().await;
    let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let result = timeout(Duration::from_secs(1), async {
        for grant in [
            String::new(),
            "a".repeat(63),
            "A".repeat(64),
            "z".repeat(64),
            "a".repeat(65),
        ] {
            assert!(matches!(
                open(
                    &phone,
                    silent.local_addr().unwrap(),
                    [0; 32],
                    &grant,
                    &CancellationToken::new()
                )
                .await,
                Err(Failure("peer-input"))
            ));
        }
        let mut bytes = [0; 2048];
        assert!(
            timeout(Duration::from_millis(100), silent.recv_from(&mut bytes))
                .await
                .is_err()
        );
    })
    .await;
    shutdown(&[&phone]).await;
    result.expect("malformed input must fail immediately");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_computer_pin_is_terminal_without_a_grant_bearing_stream() {
    let phone = node().await;
    let computer = node().await;
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let mut wrong = pin(&computer);
    wrong[0] ^= 1;
    let result = timeout(Duration::from_secs(5), async {
        let observe = async {
            let (_, connection) = computer.accept_with_connection().await.unwrap();
            assert!(
                connection.accept_bi().await.is_err(),
                "wrong pin must not disclose an app grant"
            );
        };
        let stop = CancellationToken::new();
        let (result, ()) = tokio::join!(
            open(&phone, computer.local_addr().unwrap(), wrong, &grant, &stop),
            observe
        );
        assert!(matches!(result, Err(Failure("pin-mismatch"))));
    })
    .await;
    shutdown(&[&phone, &computer]).await;
    result.expect("bounded pin rejection");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rejected_grant_is_terminal_and_never_opens_the_target() {
    let phone = node().await;
    let computer = node().await;
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    let (inspector, grant) = HttpInspector::new(port).unwrap();
    let _grant = Zeroizing::new(grant);
    let (_, wrong) = HttpInspector::new(port).unwrap();
    let wrong = Zeroizing::new(wrong);
    let result = timeout(Duration::from_secs(5), async {
        let helper = async {
            let (_, connection) = computer.accept_with_connection().await.unwrap();
            let mut counts = Counts::default();
            assert!(matches!(
                crate::application::serve_peer(connection, inspector, port, &mut counts).await,
                Err(Failure("application-auth-denied"))
            ));
            assert!(!counts.authorized);
            assert_eq!(counts.fetched, 0);
        };
        let stop = CancellationToken::new();
        let (result, ()) = tokio::join!(
            open(
                &phone,
                computer.local_addr().unwrap(),
                pin(&computer),
                &wrong,
                &stop
            ),
            helper
        );
        assert!(result.is_err(), "grant rejection must not permit fallback");
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
    })
    .await;
    shutdown(&[&phone, &computer]).await;
    result.expect("bounded grant rejection");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn atomic_abort_cannot_report_silence_after_a_peer_response() {
    let phone = node().await;
    let computer = node().await;
    let result = timeout(Duration::from_secs(5), async {
        let mut connecting = phone.begin_connect(computer.local_addr().unwrap()).unwrap();
        connecting.first_peer_response().await.unwrap();
        let (_, server) = computer.accept_with_connection().await.unwrap();
        assert!(matches!(connecting.abort(), Ok(true)));
        server.closed().await;
    })
    .await;
    shutdown(&[&phone, &computer]).await;
    result.expect("response-aware atomic abort");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn atomic_abort_preserves_an_already_recorded_error_instead_of_reporting_silence() {
    let phone = node().await;
    let server = crate::test_transport::Fixture::application_loopback()
        .await
        .unwrap()
        .backend;
    let result = timeout(Duration::from_secs(5), async {
        let mut connecting = phone
            .begin_connect(server.endpoint.local_addr().unwrap())
            .unwrap();
        let reject = async {
            assert!(server.endpoint.accept().await.unwrap().await.is_err());
        };
        // A real TLS negotiation rejection: handshake_data waits without consuming
        // Connecting, allowing the atomic abort to inspect the recorded error.
        let (data, ()) = tokio::join!(connecting.handshake_data(), reject);
        assert!(data.is_err());
        assert!(connecting.abort().is_err());
    })
    .await;
    crate::test_transport::close(&server.endpoint).await;
    shutdown(&[&phone]).await;
    result.expect("recorded TLS failure must survive atomic abort");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_while_waiting_for_the_application_ack_closes_the_checked_peer() {
    let phone = node().await;
    let computer = node().await;
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let stop = CancellationToken::new();
    let result = timeout(Duration::from_secs(5), async {
        let observe = async {
            let (_, peer) = computer.accept_with_connection().await.unwrap();
            let held = peer.accept_bi().await.unwrap();
            stop.cancel();
            peer.closed().await;
            drop(held);
        };
        let (result, ()) = tokio::join!(
            open(
                &phone,
                computer.local_addr().unwrap(),
                pin(&computer),
                &grant,
                &stop
            ),
            observe
        );
        assert!(matches!(result, Err(Failure("peer-cancelled"))));
    })
    .await;
    shutdown(&[&phone, &computer]).await;
    result.expect("cancellation during application authorization");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn already_cancelled_and_unpolled_attempts_send_no_datagrams() {
    let phone = node().await;
    let silent = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = silent.local_addr().unwrap();
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let stop = CancellationToken::new();
    stop.cancel();
    assert!(matches!(
        open(&phone, address, [0; 32], &grant, &stop).await,
        Err(Failure("peer-cancelled"))
    ));
    drop(open(
        &phone,
        address,
        [0; 32],
        &grant,
        &CancellationToken::new(),
    ));
    let mut bytes = [0; 2048];
    let received = timeout(Duration::from_millis(100), silent.recv_from(&mut bytes)).await;
    shutdown(&[&phone]).await;
    assert!(received.is_err());
}
