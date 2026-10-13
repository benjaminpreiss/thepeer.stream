//! Authorization/Stop ordering through real offers, TLS, packet loss and browser traffic.
//! No grant extraction, private state setters, fabricated TLS or public participants.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};

// Preserve genuine QUIC-v1 Initial/Handshake packets; hold only encrypted 1-RTT
// tails. These are trusted local transport packets, not a hostile-packet parser.
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
        assert_eq!(&packet[offset + 1..offset + 5], &[0, 0, 0, 1]);
        assert!(kind == 0 || kind == 2);
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

#[derive(Clone, Copy, PartialEq)]
enum Case {
    Release,
    ClientRelease,
    AfterConnectStop,
    PhoneStop,
    PhoneDrop,
    HelperStop,
    Race,
}

async fn authorization_edge(case: Case, stop_first: bool) {
    timeout(Duration::from_secs(40), async {
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let helper_stop = CancellationToken::new();
        let _helper_guard = helper_stop.clone().drop_guard();
        let (mut helper, mut offer) = Helper::prepare(HelperConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            target: format!("localhost:{}", target.local_addr().unwrap().port()).parse().unwrap(), observer: None,
        }, &helper_stop).await.unwrap();
        let helper_bind = offer.direct_candidate();
        let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let phone_bind = reservation.local_addr().unwrap();
        drop(reservation);
        let proxy = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        offer.set_direct_candidate(proxy.local_addr().unwrap());
        let decoded = Offer::decode(&offer.encode().unwrap()).unwrap();
        drop(offer);
        let network_stop = CancellationToken::new();
        let _network_guard = network_stop.clone().drop_guard();
        let release = CancellationToken::new();
        let held_count = Arc::new(AtomicUsize::new(0));
        let proxy_count = held_count.clone();
        let done = network_stop.clone();
        let released = release.clone();
        let forwarding = tokio::spawn(async move {
            let mut packet = vec![0;65536];
            let mut held = Vec::<(Vec<u8>, SocketAddr)>::new();
            let mut bytes = 0;
            let mut open = false;
            loop {
                tokio::select! {
                    biased;
                    _ = done.cancelled() => break,
                    _ = released.cancelled(), if !open => {
                        open = true;
                        for (packet, address) in held.drain(..) { proxy.send_to(&packet, address).await.unwrap(); }
                    },
                    received = proxy.recv_from(&mut packet) => {
                        let (len, source) = received.unwrap();
                        assert!(source == helper_bind || source == phone_bind);
                        let address = if source == helper_bind { phone_bind } else { helper_bind };
                        let hold_direction = if matches!(case, Case::HelperStop | Case::ClientRelease) { source == phone_bind } else { source == helper_bind };
                        let prefix = if !open && hold_direction { handshake_prefix(&packet[..len]) } else { len };
                        if prefix > 0 { proxy.send_to(&packet[..prefix], address).await.unwrap(); }
                        if prefix < len {
                            bytes += len - prefix;
                            assert!(held.len() < 32 && bytes <= 128 * 1024, "bounded encrypted packet hold");
                            held.push((packet[prefix..len].to_vec(), address));
                            proxy_count.fetch_add(1, Ordering::SeqCst);
                        }
                    },
                }
            }
        });
        let phone_stop = CancellationToken::new();
        let _phone_guard = phone_stop.clone().drop_guard();
        let connect_stop = phone_stop.clone();
        let connecting = tokio::spawn(async move { connect(decoded, phone_bind, &connect_stop).await });
        timeout(Duration::from_secs(4), async {
            loop {
                let progress = helper.progress();
                let phase = if matches!(case, Case::HelperStop | Case::ClientRelease) { progress.admitted == 1 && !progress.authorized } else { progress.authorized };
                if phase && held_count.load(Ordering::SeqCst) > 0 { break; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.expect("real TLS/application phase plus held encrypted traffic");
        assert!(!connecting.is_finished(), "phone must still await authorization acknowledgment");
        assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        let mut connected = None;
        match case {
            Case::Release | Case::ClientRelease | Case::AfterConnectStop => {
                release.cancel();
                connected = Some(timeout(Duration::from_secs(4), connecting).await.unwrap().unwrap().unwrap());
                if case == Case::AfterConnectStop { phone_stop.cancel(); }
            },
            Case::PhoneStop => {
                phone_stop.cancel();
                let result = timeout(Duration::from_secs(8), connecting).await.expect("Stop must beat authorization timeout").unwrap();
                assert!(matches!(result, Err(Failure("peer-cancelled"))));
                release.cancel();
            },
            Case::PhoneDrop => {
                connecting.abort();
                let result = timeout(Duration::from_secs(8), connecting).await.unwrap();
                assert!(matches!(result, Err(error) if error.is_cancelled()));
                assert!(!phone_stop.is_cancelled());
                release.cancel();
            },
            Case::HelperStop => {
                helper_stop.cancel();
                release.cancel();
                timeout(Duration::from_secs(8), helper.wait()).await.unwrap().unwrap();
                assert!(!helper.progress().authorized, "Stop before held client traffic must prevent late authorization");
                let result = timeout(Duration::from_secs(8), connecting).await.unwrap().unwrap();
                match result {
                    Err(error) => assert_ne!(error.0, "route-unavailable", "authorization closure is terminal, not fallback"),
                    Ok(session) => { drop(session); panic!("stopped helper must not authorize phone"); }
                }
            },
            Case::Race => {
                // Two public events; no forced internal interleaving or claimed fairness.
                if stop_first { phone_stop.cancel(); release.cancel(); }
                else { release.cancel(); phone_stop.cancel(); }
                let result = timeout(Duration::from_secs(8), connecting).await.unwrap().unwrap();
                match result {
                    Err(error) => assert_eq!(error.0, "peer-cancelled"),
                    Ok(session) => { assert!(!stop_first); connected = Some(session); }
                }
            },
        }
        if let Some(connected) = connected {
            assert_eq!(connected.route(), Route::Direct);
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let hostname = format!("thepeer-{}.localhost", hex::encode(rand::random::<[u8;16]>()));
            let mut browser = TcpStream::connect(address).await.unwrap();
            browser.write_all(format!("GET /fixture HTTP/1.1\r\nHost: {hostname}:{}\r\nConnection: close\r\n\r\n", address.port()).as_bytes()).await.unwrap();
            let serving_stop = CancellationToken::new();
            let serve_token = serving_stop.clone();
            let serving = tokio::spawn(async move { connected.serve(listener, hostname, &serve_token).await });
            if matches!(case, Case::Release | Case::ClientRelease) {
                let (mut stream, _) = timeout(Duration::from_secs(4), target.accept()).await.unwrap().unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") { assert!(header.len() < 16384); header.push(stream.read_u8().await.unwrap()); }
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nok").await.unwrap();
                drop(stream);
                let mut response = Vec::new();
                timeout(Duration::from_secs(4), browser.read_to_end(&mut response)).await.unwrap().unwrap();
                let response = String::from_utf8(response).unwrap();
                assert!(response.starts_with("HTTP/1.1 200"));
                assert!(!response.to_ascii_lowercase().contains("content-security-policy:"));
                assert!(response.ends_with("ok"));
                phone_stop.cancel();
            }
            tokio::select! {
                _ = target.accept() => panic!("stopped authorized session must not contact target"),
                result = timeout(Duration::from_secs(8), serving) => { result.unwrap().unwrap().unwrap(); },
            }
            assert!(!serving_stop.is_cancelled(), "setup Stop alone must close the gateway");
            assert!(TcpStream::connect(address).await.is_err());
        }
        // Even late ciphertext after Stop/drop must not open a target connection.
        assert!(timeout(Duration::from_millis(100), target.accept()).await.is_err());
        let _phone_released = timeout(Duration::from_secs(10), async {
            loop {
                match UdpSocket::bind(phone_bind).await {
                    Ok(socket) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => tokio::time::sleep(Duration::from_millis(20)).await,
                    Err(error) => panic!("unexpected rebind failure: {error}"),
                }
            }
        }).await.expect("phone ownership must drain");
        if case != Case::HelperStop { assert!(!helper_stop.is_cancelled()); }
        timeout(Duration::from_secs(12), helper.shutdown()).await.unwrap().unwrap();
        let _helper_released = UdpSocket::bind(helper_bind).await.unwrap();
        network_stop.cancel();
        forwarding.await.unwrap();
    }).await.expect("bounded authorization/Stop fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn held_authorization_ack_releases_into_real_browser_http() {
    authorization_edge(Case::Release, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn held_client_application_traffic_releases_into_authorized_browser_http() {
    authorization_edge(Case::ClientRelease, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_after_authorization_before_serve_rejects_queued_browser() {
    authorization_edge(Case::AfterConnectStop, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn phone_stop_while_authorization_ack_is_held_is_terminal() {
    authorization_edge(Case::PhoneStop, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropping_phone_setup_after_helper_authorization_releases_owner() {
    authorization_edge(Case::PhoneDrop, false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn helper_stop_before_held_client_application_traffic_prevents_authorization() {
    authorization_edge(Case::HelperStop, true).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authorization_ack_release_racing_stop_never_opens_target() {
    for stop_first in [true, false, true, false] {
        authorization_edge(Case::Race, stop_first).await;
    }
}
