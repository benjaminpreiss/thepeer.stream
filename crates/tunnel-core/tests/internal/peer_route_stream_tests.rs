use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropping_active_relay_browser_serving_closes_sse_target_and_phone_socket() {
    timeout(Duration::from_secs(30), async {
        let relay = P2pEndpoint::new(P2pConfig::builder()
            .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .nat(NatConfig { allow_loopback: true, enable_relay_service: true,
                advertise_external_addresses: false, upnp: UpnpConfig::disabled(),
                ..NatConfig::default() }).build().unwrap()).await.unwrap();
        // Distinct tokens: killing the phone's serving future must not rely on
        // cancelling the helper or the setup token to close its target stream.
        let helper_stop = CancellationToken::new();
        let phone_stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, mut offer) = Helper::prepare_with_relay(HelperConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(), observer: None,
        }, relay.local_addr().unwrap(), &helper_stop).await.unwrap();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.set_direct_candidate(quiet.local_addr().unwrap());
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone_address = reservation.local_addr().unwrap();
        drop(reservation);
        let connected = connect(offer, phone_address, &phone_stop).await.unwrap();
        assert_eq!(connected.route(), Route::Relay);
        assert!(UdpSocket::bind(phone_address).await.is_err());
        let mut target_tasks = JoinSet::new();
        target_tasks.spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(socket.read_u8().await.unwrap());
            }
            assert!(header.starts_with(b"GET /events HTTP/1.1\r\n"));
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\nd\r\ndata: ready\n\n\r\n").await.unwrap();
            // Deliberately leave the response unfinished. Only caller teardown
            // should end this connection; no fixture deadline closes it for us.
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
            assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let browser_address = listener.local_addr().unwrap();
        let hostname = format!("thepeer-{}.localhost", hex::encode(rand::random::<[u8;16]>()));
        let authority = format!("{hostname}:{}", browser_address.port());
        let mut browser_tasks = JoinSet::new();
        let serving_stop = phone_stop.clone();
        browser_tasks.spawn(async move { connected.serve(listener, hostname, &serving_stop).await });
        let mut response = reqwest::Client::builder().no_proxy().build().unwrap()
            .get(format!("http://{browser_address}/events")).header("Host", authority).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let mut event = Vec::new();
        timeout(Duration::from_secs(5), async {
            while event.len() < 13 { event.extend_from_slice(&response.chunk().await.unwrap().unwrap()); }
        }).await.unwrap();
        assert_eq!(event, b"data: ready\n\n");
        assert!(!target_tasks.is_empty());
        browser_tasks.abort_all();
        assert!(browser_tasks.join_next().await.unwrap().unwrap_err().is_cancelled());
        timeout(Duration::from_secs(5), target_tasks.join_next()).await
            .expect("dropping serving must close the active target before helper shutdown")
            .unwrap().unwrap();
        timeout(Duration::from_secs(5), async {
            loop {
                match UdpSocket::bind(phone_address).await {
                    Ok(socket) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => tokio::time::sleep(Duration::from_millis(10)).await,
                    Err(error) => panic!("unexpected phone bind error: {error}"),
                }
            }
        }).await.expect("phone runtime must release its UDP listener");
        assert!(tokio::net::TcpStream::connect(browser_address).await.is_err());
        assert!(!phone_stop.is_cancelled());
        assert!(!helper_stop.is_cancelled());
        // Closure/reset is acceptable; an unfinished SSE response must not keep
        // delivering data once its browser serving task has been dropped.
        assert!(!matches!(timeout(Duration::from_secs(2), response.chunk()).await.unwrap(), Ok(Some(_))));
        helper.shutdown().await.unwrap();
        timeout(Duration::from_secs(8), relay.shutdown()).await.unwrap();
    }).await.expect("bounded active relay drop fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropping_helper_during_relay_websocket_closes_both_streams_without_reconnect() {
    timeout(Duration::from_secs(35), async {
        let relay = P2pEndpoint::new(P2pConfig::builder()
            .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .nat(NatConfig { allow_loopback: true, enable_relay_service: true,
                advertise_external_addresses: false, upnp: UpnpConfig::disabled(),
                ..NatConfig::default() }).build().unwrap()).await.unwrap();
        let helper_stop = CancellationToken::new();
        let phone_stop = CancellationToken::new();
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (helper, mut offer) = Helper::prepare_with_relay(HelperConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(), observer: None,
        }, relay.local_addr().unwrap(), &helper_stop).await.unwrap();
        let helper_address = offer.direct_candidate();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.set_direct_candidate(quiet.local_addr().unwrap());
        let connected = connect(offer, "127.0.0.1:0".parse().unwrap(), &phone_stop).await.unwrap();
        assert_eq!(connected.route(), Route::Relay);
        let mut target_tasks = JoinSet::new();
        target_tasks.spawn(async move {
            let (mut socket, _) = target.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                assert!(header.len() < 16384);
                header.push(socket.read_u8().await.unwrap());
            }
            assert!(header.starts_with(b"GET /socket HTTP/1.1\r\n"));
            socket.write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n").await.unwrap();
            let mut frame = [0; 10];
            socket.read_exact(&mut frame).await.unwrap();
            assert_eq!(frame, [0x82, 0x84, 1, 2, 3, 4, 1, 253, 124, 132]);
            socket.write_all(&[0x82, 4, 0, 255, 127, 128]).await.unwrap();
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).await.unwrap(), 0);
            assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let browser_address = listener.local_addr().unwrap();
        let hostname = format!("thepeer-{}.localhost", hex::encode(rand::random::<[u8;16]>()));
        let authority = format!("{hostname}:{}", browser_address.port());
        let mut browser_tasks = JoinSet::new();
        let serving_stop = phone_stop.clone();
        browser_tasks.spawn(async move { connected.serve(listener, hostname, &serving_stop).await });
        let mut browser = tokio::net::TcpStream::connect(browser_address).await.unwrap();
        browser.write_all(format!("GET /socket HTTP/1.1\r\nHost: {authority}\r\nOrigin: http://{authority}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").as_bytes()).await.unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            assert!(header.len() < 16384);
            header.push(browser.read_u8().await.unwrap());
        }
        assert!(header.starts_with(b"HTTP/1.1 101"));
        browser.write_all(&[0x82, 0x84, 1, 2, 3, 4, 1, 253, 124, 132]).await.unwrap();
        let mut frame = [0; 6];
        browser.read_exact(&mut frame).await.unwrap();
        assert_eq!(frame, [0x82, 4, 0, 255, 127, 128]);
        drop(helper);
        let mut byte = [0];
        assert_eq!(timeout(Duration::from_secs(5), browser.read(&mut byte)).await.unwrap().unwrap(), 0);
        timeout(Duration::from_secs(5), target_tasks.join_next()).await.unwrap().unwrap().unwrap();
        let result = timeout(Duration::from_secs(8), browser_tasks.join_next()).await.unwrap().unwrap().unwrap();
        assert!(matches!(result, Err(Failure("application-helper-closed"))));
        timeout(Duration::from_secs(15), async {
            loop {
                match UdpSocket::bind(helper_address).await {
                    Ok(socket) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => tokio::time::sleep(Duration::from_millis(10)).await,
                    Err(error) => panic!("unexpected helper bind error: {error}"),
                }
            }
        }).await.expect("dropped helper must release its own UDP listener");
        assert!(tokio::net::TcpStream::connect(browser_address).await.is_err());
        assert!(!phone_stop.is_cancelled());
        assert!(!helper_stop.is_cancelled());
        assert!(relay.is_running());
        timeout(Duration::from_secs(8), relay.shutdown()).await.unwrap();
    }).await.expect("bounded helper drop websocket fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_during_observation_is_terminal_with_relay_available() {
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
        let helper_stop = CancellationToken::new();
        let phone_stop = CancellationToken::new();
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
            &helper_stop,
        )
        .await
        .unwrap();
        let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.observer = Some(quiet.local_addr().unwrap());
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone_address = reservation.local_addr().unwrap();
        drop(reservation);
        let mut tasks = JoinSet::new();
        let setup_stop = phone_stop.clone();
        tasks.spawn(async move { connect(offer, phone_address, &setup_stop).await });
        let mut packet = [0; 65536];
        let (size, source) = timeout(Duration::from_secs(5), quiet.recv_from(&mut packet))
            .await
            .unwrap()
            .unwrap();
        assert!(size > 0);
        assert_eq!(source, phone_address);
        // Receipt, rather than a sleep, establishes that setup is actually active.
        phone_stop.cancel();
        // connect returns after owned transport cleanup, including upstream's
        // bounded QUIC drain. This is not a two-second UI responsiveness test.
        let result = timeout(Duration::from_secs(10), tasks.join_next())
            .await
            .expect("cancelled setup must finish owned cleanup")
            .unwrap()
            .unwrap();
        assert!(matches!(result, Err(Failure("peer-cancelled"))));
        let _released = UdpSocket::bind(phone_address).await.unwrap();
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
        assert!(!helper_stop.is_cancelled());
        helper.shutdown().await.unwrap();
        timeout(Duration::from_secs(8), relay.shutdown())
            .await
            .unwrap();
    })
    .await
    .expect("bounded setup cancellation fixture");
}
