use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_during_final_relay_dial_closes_phone_without_authorization_or_target_access() {
    timeout(Duration::from_secs(65), async {
        // Positive control uses the identical relay/proxy/codec path. The two
        // faults withhold all replies or stop TLS after one genuine reply.
        for mode in ["pass", "silent", "partial"] {
            let relay = P2pEndpoint::new(P2pConfig::builder()
                .bind_addr("127.0.0.1:0".parse::<SocketAddr>().unwrap())
                .nat(NatConfig { allow_loopback: true, enable_relay_service: true,
                    advertise_external_addresses: false, upnp: UpnpConfig::disabled(), ..NatConfig::default() })
                .build().unwrap()).await.unwrap();
            let helper_stop = CancellationToken::new();
            let phone_stop = CancellationToken::new();
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (helper, mut offer) = Helper::prepare_with_relay(HelperConfig {
                bind: "127.0.0.1:0".parse().unwrap(), target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(), observer: None,
            }, relay.local_addr().unwrap(), &helper_stop).await.unwrap();
            let computer = offer.direct_candidate();
            let contact = offer.relay_candidate().unwrap();
            let quiet = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            offer.set_direct_candidate(quiet.local_addr().unwrap());
            let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let proxy_addr = proxy.local_addr().unwrap();
            assert_eq!(proxy_addr.ip(), contact.ip());
            // Alter only a public routing hint through the sole wire codec.
            // Pin/grant are real, unmodified, and never logged or persisted.
            let mut text = offer.encode().unwrap();
            let mut packet = Zeroizing::new(URL_SAFE_NO_PAD.decode(&text[5..]).unwrap());
            assert_eq!(packet[1], 14, "relay, bound and observer flags");
            assert_eq!(packet[66], 4, "IPv4 Direct record");
            assert_eq!(packet[73], 4, "IPv4 relay record");
            // Leave the new bound/observer records following Relay untouched.
            packet[78..80].copy_from_slice(&proxy_addr.port().to_be_bytes());
            text = Zeroizing::new("wpa2:".to_owned());
            URL_SAFE_NO_PAD.encode_string(&*packet, &mut text);
            let decoded = Offer::decode(&text).unwrap();
            assert_eq!(decoded.relay_candidate(), Some(proxy_addr));
            drop((offer, text, packet));

            let reserve = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let phone = reserve.local_addr().unwrap();
            drop(reserve);
            let network_stop = CancellationToken::new();
            let network_done = network_stop.clone();
            let (held_tx, held_rx) = oneshot::channel();
            let proxy_task = tokio::spawn(async move {
                let mut held = Some(held_tx);
                let mut replies = 0;
                let mut packet = [0; 65536];
                loop {
                    let (length, source) = tokio::select! {
                        _ = network_done.cancelled() => break,
                        result = proxy.recv_from(&mut packet) => result.unwrap(),
                    };
                    if source == contact {
                        replies += 1;
                        if mode == "pass" || (mode == "partial" && replies == 1) {
                            proxy.send_to(&packet[..length], phone).await.unwrap();
                        } else if let Some(held) = held.take() {
                            // An actual virtual helper TLS reply reached the
                            // allocated contact and proxy, not a mocked attempt.
                            let _ = held.send(replies);
                        }
                    } else {
                        assert_eq!(source, phone);
                        proxy.send_to(&packet[..length], contact).await.unwrap();
                    }
                }
            });
            let setup_stop = phone_stop.clone();
            let connecting = tokio::spawn(async move { connect(decoded, phone, &setup_stop).await });
            if mode != "pass" {
                let replies = timeout(Duration::from_secs(15), held_rx).await
                    .expect("final relay dial must reach the real helper").unwrap();
                assert_eq!(replies, if mode == "silent" { 1 } else { 2 });
                assert!(UdpSocket::bind(phone).await.is_err(), "phone still owns its relay endpoint");
                phone_stop.cancel();
                let result = timeout(Duration::from_secs(10), connecting).await.unwrap().unwrap();
                let stage = result.as_ref().err().map(|error| error.0);
                drop(result);
                assert_eq!(stage, Some("peer-cancelled"), "Stop must not become unavailability or a session");
                assert!(!helper.progress().authorized);
                assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
            } else {
                let connected = timeout(Duration::from_secs(15), connecting).await.unwrap().unwrap().unwrap();
                assert_eq!(connected.route(), Route::Relay);
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let browser = listener.local_addr().unwrap();
                let hostname = format!("thepeer-{}.localhost", hex::encode(rand::random::<[u8; 16]>()));
                let authority = format!("{hostname}:{}", browser.port());
                let serve_stop = phone_stop.clone();
                let serve = tokio::spawn(async move { connected.serve(listener, hostname, &serve_stop).await });
                let target_task = tokio::spawn(async move {
                    let (mut socket, _) = target.accept().await.unwrap();
                    let mut header = Vec::new();
                    while !header.ends_with(b"\r\n\r\n") {
                        assert!(header.len() < 16384);
                        header.push(socket.read_u8().await.unwrap());
                    }
                    assert!(header.starts_with(b"GET /relay-stop HTTP/1.1\r\n"));
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ncontrol").await.unwrap();
                });
                let response = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5))
                    .build().unwrap().get(format!("http://{browser}/relay-stop"))
                    .header("Host", authority).send().await.unwrap();
                assert_eq!(response.status(), 200);
                assert!(!response.headers().contains_key("content-security-policy"));
                assert_eq!(response.text().await.unwrap(), "control");
                target_task.await.unwrap();
                phone_stop.cancel();
                timeout(Duration::from_secs(10), serve).await.unwrap().unwrap().unwrap();
                assert!(tokio::net::TcpStream::connect(browser).await.is_err());
            }
            // Private Direct is ineligible without a shared public observation.
            // The explicit phone port is released after Relay, with no reconnect.
            assert!(quiet.try_recv_from(&mut [0; 2048]).is_err());
            let _phone_released = UdpSocket::bind(phone).await.unwrap();
            assert!(!helper_stop.is_cancelled());
            assert!(relay.is_running());
            helper.shutdown().await.unwrap();
            let _computer_released = UdpSocket::bind(computer).await.unwrap();
            network_stop.cancel();
            proxy_task.await.unwrap();
            timeout(Duration::from_secs(8), relay.shutdown()).await.unwrap();
        }
    }).await.expect("bounded final relay cancellation fixture");
}
