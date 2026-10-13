//! Real phone ingress regressions, promoted from the bounded Direct diagnostic.
use crate::peer_route::{Helper, HelperConfig, Offer, Route, connect};
use saorsa_transport::{NatConfig, P2pConfig, P2pEndpoint, upnp::UpnpConfig};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
    task::JoinSet,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

async fn healthy_phone_refuses_unsolicited_tls(route: Route, abandon: bool) {
    timeout(Duration::from_secs(45), async {
        let helper_stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let participant = if route == Route::Direct { None } else { Some(node(route == Route::Relay).await) };
        let config = HelperConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(),
            observer: None,
        };
        let (helper, mut offer) = if route == Route::Relay {
            Helper::prepare_with_relay(config, participant.as_ref().unwrap().local_addr().unwrap(), &helper_stop).await.unwrap()
        } else { Helper::prepare(config, &helper_stop).await.unwrap() };
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        if route != Route::Direct { offer.set_direct_candidate(quiet.local_addr().unwrap()); }
        let decoded = Offer::decode(&offer.encode().unwrap()).unwrap();
        drop(offer);
        let reserved = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let bind = reserved.local_addr().unwrap();
        drop(reserved);
        let stop = CancellationToken::new();
        let _stop_on_unwind = stop.clone().drop_guard();
        let connected = connect(decoded, bind, &stop).await.unwrap();
        assert_eq!(connected.route(), route);
        let browser = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let browser_addr = browser.local_addr().unwrap();
        let hostname = format!("thepeer-{}.localhost", hex::encode(rand::random::<[u8;16]>()));
        let authority = format!("{hostname}:{}", browser_addr.port());
        let serve_stop = stop.clone();
        let serving = tokio::spawn(async move { connected.serve(browser, hostname, &serve_stop).await });
        let attacker = node(false).await;
        let attacker_addr = attacker.local_addr().unwrap();
        let network_stop = CancellationToken::new();
        let mut forwarding = JoinSet::new();
        let mut peers = Vec::new();
        let mut pending = Vec::new();
        let mut refused = 0;
        for _ in 0..5 {
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let address = proxy.local_addr().unwrap();
            let done = network_stop.clone();
            forwarding.spawn(async move {
                let mut responded = false;
                let mut packet = vec![0;65536];
                loop {
                    let (len, source) = tokio::select! {
                        _ = done.cancelled() => break,
                        received = proxy.recv_from(&mut packet) => received.unwrap(),
                    };
                    if source == bind {
                        responded = true;
                        proxy.send_to(&packet[..len], attacker_addr).await.unwrap();
                    } else {
                        assert_eq!(source, attacker_addr);
                        // Genuine server flights; withhold all client traffic after
                        // first response, including Finished and retransmissions.
                        if !responded { proxy.send_to(&packet[..len], bind).await.unwrap(); }
                    }
                }
            });
            let mut attempt = attacker.begin_connect(address).unwrap();
            match timeout(Duration::from_millis(700), &mut attempt).await {
                Ok(Ok(peer)) => peers.push(peer),
                Ok(Err(saorsa_transport::ConnectionError::ConnectionClosed(error)))
                    if error.error_code == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED => { refused += 1; }
                Ok(Err(error)) => panic!("unexpected ingress outcome: {error}"),
                Err(_) => pending.push(attempt),
            }
        }
        let completed = peers.len();
        let all_live = peers.iter().all(|peer| peer.close_reason().is_none());
        let no_target = timeout(Duration::from_millis(100), target.accept()).await.is_err();
        // The legitimate, already-authorized route still serves real browser HTTP.
        let target_task = tokio::spawn(async move {
            let (mut stream, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(stream.read_u8().await.unwrap());
            }
            assert!(header.starts_with(b"GET /fixture HTTP/1.1\r\n"));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nok").await.unwrap();
        });
        let response = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap()
            .get(format!("http://{browser_addr}/fixture")).header("Host", authority).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("content-security-policy"));
        assert_eq!(response.text().await.unwrap(), "ok");
        target_task.await.unwrap();
        // Clean up before the admission assertion, including on the expected red.
        if abandon {
            serving.abort();
            assert!(timeout(Duration::from_secs(18), serving).await.unwrap().unwrap_err().is_cancelled());
            assert!(!stop.is_cancelled(), "dropping the session must not cancel its parent");
        } else {
            stop.cancel();
            timeout(Duration::from_secs(18), serving).await.unwrap().unwrap().unwrap();
        }
        let _released = timeout(Duration::from_secs(18), async {
            loop {
                match UdpSocket::bind(bind).await {
                    Ok(socket) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => tokio::time::sleep(Duration::from_millis(20)).await,
                    Err(error) => panic!("unexpected rebind failure: {error}"),
                }
            }
        }).await.expect("phone stop/drop releases UDP");
        assert!(tokio::net::TcpStream::connect(browser_addr).await.is_err());
        assert!(!helper_stop.is_cancelled());
        helper.shutdown().await.unwrap();
        if let Some(participant) = participant {
            assert!(participant.is_running());
            timeout(Duration::from_secs(8), participant.shutdown()).await.unwrap();
        }
        for peer in peers { peer.close(0u32.into(), b"probe complete"); }
        for attempt in pending { let _ = attempt.abort(); }
        network_stop.cancel();
        while let Some(done) = forwarding.join_next().await { done.unwrap(); }
        timeout(Duration::from_secs(8), attacker.shutdown()).await.unwrap();
        assert!(no_target, "unsolicited TLS must not access the target");
        assert_eq!(completed, 0,
            "phone started unsolicited TLS: {completed} client completions with phone-side Finished withheld (all live={all_live}, {refused} refused)");
        assert_eq!(refused, 5, "closed phone ingress should explicitly refuse all five attempts");
    }).await.expect("bounded five-attempt phone ingress fixture");
}

async fn node(serves_relay: bool) -> P2pEndpoint {
    P2pEndpoint::new(
        P2pConfig::builder()
            .bind_addr("127.0.0.1:0".parse::<std::net::SocketAddr>().unwrap())
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn healthy_direct_phone_refuses_unsolicited_tls_and_stops() {
    healthy_phone_refuses_unsolicited_tls(Route::Direct, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn healthy_relay_phone_refuses_unsolicited_tls_and_stops() {
    healthy_phone_refuses_unsolicited_tls(Route::Relay, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropping_healthy_phone_after_refused_tls_releases_owner() {
    healthy_phone_refuses_unsolicited_tls(Route::Direct, true).await;
}
