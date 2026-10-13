//! Fixed-budget paced overload, not an indefinite flood or fairness benchmark.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paced_mixed_ingress_contention_preserves_queue_recovery_and_browser_http() {
    timeout(Duration::from_secs(85), async {
        let relay = node(true).await;
        let attacker = node(false).await;
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stop = CancellationToken::new();
        let _stop_guard = stop.clone().drop_guard();
        let (helper, offer) = Helper::prepare_with_relay(HelperConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(), observer: None,
        }, relay.local_addr().unwrap(), &stop).await.unwrap();
        let decoded = Offer::decode(&offer.encode().unwrap()).unwrap();
        drop(offer);
        let helper_bind = decoded.direct_candidate();
        let mut contacts = [helper_bind, decoded.relay_candidate().unwrap()];
        let mut refused = [0; 2];
        for _ in 0..2 {
            let started = tokio::time::Instant::now();
            let wave = MixedWave::fill(&attacker, contacts).await;
            // Keep four real handshakes unfinished and four waiting, while fresh
            // Initials arrive on both ingresses. Sleep AFTER each result: no
            // catch-up burst, at most eight fresh attempts/sec, forty per window.
            for probe in 0..40 {
                let ingress = probe % 2;
                let attempt = attacker.begin_connect(contacts[ingress]).unwrap();
                let result = timeout(Duration::from_secs(1), attempt).await
                    .expect("overflow must return a bounded refusal under paced load");
                assert!(matches!(result,
                    Err(saorsa_transport::ConnectionError::ConnectionClosed(ref error))
                    if error.error_code == saorsa_transport::TransportErrorCode::CONNECTION_REFUSED),
                    "fresh overflow must not enter TLS or silently queue");
                refused[ingress] += 1;
                assert!(wave.replies[4..].iter().all(|count| count.load(Ordering::SeqCst) == 0),
                    "fresh arrivals must not start or evict already queued work");
                assert_eq!(helper.progress().admitted, 0);
                assert!(!helper.progress().authorized);
                tokio::time::sleep(Duration::from_millis(125)).await;
                // An expired active slot would invalidate the saturation premise.
                // Leave two seconds of headroom before the ten-second TLS deadline.
                assert!(started.elapsed() < Duration::from_secs(8),
                    "fixture load window must finish before active TLS expiry");
            }
            assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
            // Require original queued clients to complete genuine client-side
            // TLS on promotion, not receive a late refusal, then expire normally.
            wave.expire().await;
            contacts.reverse();
        }
        assert_eq!(refused, [40, 40]);
        assert_eq!(helper.progress().admitted, 0);
        assert!(!helper.progress().authorized);
        assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        let reserved = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone_bind = reserved.local_addr().unwrap();
        drop(reserved);
        // One explicit connection with the original decoded offer. No helper
        // restart, replacement grant or automatic legitimate-client retries.
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
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nrecovered").await.unwrap();
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let hostname = format!("thepeer-{}.localhost", hex::encode(rand::random::<[u8;16]>()));
        let authority = format!("{hostname}:{}", address.port());
        let browser_stop = stop.clone();
        serving.spawn(async move { connected.serve(listener, hostname, &browser_stop).await.unwrap(); });
        let response = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap()
            .get(format!("http://{address}/recovered")).header("Host", authority).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("content-security-policy"));
        assert_eq!(response.text().await.unwrap(), "recovered");
        assert!(helper.progress().authorized);
        stop.cancel();
        while let Some(done) = serving.join_next().await { done.unwrap(); }
        helper.shutdown().await.unwrap();
        let _helper_released = UdpSocket::bind(helper_bind).await.unwrap();
        let _phone_released = UdpSocket::bind(phone_bind).await.unwrap();
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        assert!(relay.is_running());
        timeout(Duration::from_secs(8), attacker.shutdown()).await.unwrap();
        timeout(Duration::from_secs(8), relay.shutdown()).await.unwrap();
    }).await.expect("bounded 100-attempt mixed-ingress contention fixture");
}
