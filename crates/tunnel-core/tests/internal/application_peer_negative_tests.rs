//! Authorization/lifetime regressions at the retained-peer application seam.
use super::*;
use crate::http::{host::Counts, inspector::HttpInspector};
use saorsa_transport::crypto::{
    pqc::types::MlDsaPublicKey, raw_public_keys::fingerprint_public_key,
};
use saorsa_transport::{NatConfig, P2pConfig, P2pEndpoint, upnp::UpnpConfig};

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
struct Pair {
    phone: P2pEndpoint,
    computer: P2pEndpoint,
    client: HighLevelConnection,
    server: HighLevelConnection,
    pin: [u8; 32],
}
impl Pair {
    async fn new() -> Self {
        let computer = node().await;
        let phone = node().await;
        let addr = computer.local_addr().unwrap();
        phone.connect(addr).await.unwrap();
        let peer = computer.accept().await.unwrap();
        let server = computer
            .get_quic_connection(&peer.remote_addr.as_socket_addr().unwrap())
            .await
            .unwrap()
            .unwrap();
        let client = phone.get_quic_connection(&addr).await.unwrap().unwrap();
        let pin = fingerprint_public_key(
            &MlDsaPublicKey::from_bytes(computer.public_key_bytes()).unwrap(),
        );
        Self {
            phone,
            computer,
            client,
            server,
            pin,
        }
    }
    async fn shutdown(self) {
        self.phone.shutdown().await;
        self.computer.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_computer_pin_closes_without_opening_an_application_stream() {
    let pair = Pair::new().await;
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let mut wrong = pair.pin;
    wrong[0] ^= 1;
    let outcome = timeout(Duration::from_secs(5), async {
        let result = Session::from_peer(pair.client.clone(), wrong, &grant).await;
        assert!(matches!(result, Err(Failure("pin-mismatch"))));
        assert!(
            pair.server.accept_bi().await.is_err(),
            "no grant-bearing stream may be opened"
        );
    })
    .await;
    pair.shutdown().await;
    outcome.expect("wrong-pin close deadline");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_grant_never_opens_the_target() {
    let pair = Pair::new().await;
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = target.local_addr().unwrap().port();
    let (inspector, actual) = HttpInspector::new(port).unwrap();
    let _actual = Zeroizing::new(actual);
    let (_, wrong) = HttpInspector::new(port).unwrap();
    let wrong = Zeroizing::new(wrong);
    let outcome = timeout(Duration::from_secs(5), async {
        let mut counts = Counts::default();
        let (client, server) = tokio::join!(
            Session::from_peer(pair.client.clone(), pair.pin, &wrong),
            serve_peer(pair.server.clone(), inspector, port, &mut counts)
        );
        assert!(client.is_err());
        assert!(matches!(server, Err(Failure("application-auth-denied"))));
        assert!(!counts.authorized);
        assert_eq!(counts.fetched, 0);
        assert_eq!(counts.failed, 1);
        assert!(
            timeout(Duration::from_millis(100), target.accept())
                .await
                .is_err()
        );
    })
    .await;
    pair.shutdown().await;
    outcome.expect("wrong-grant close deadline");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_before_first_poll_closes_the_owned_connection() {
    let pair = Pair::new().await;
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let pending = Session::from_peer(pair.client.clone(), pair.pin, &grant);
    drop(pending);
    let closed = timeout(Duration::from_secs(1), pair.server.closed()).await;
    pair.shutdown().await;
    assert!(
        closed.is_ok(),
        "an unpolled setup future must release its connection"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_an_unpolled_helper_closes_the_accepted_connection() {
    let pair = Pair::new().await;
    let (inspector, grant) = HttpInspector::new(1234).unwrap();
    let _grant = Zeroizing::new(grant);
    let mut counts = Counts::default();
    drop(serve_peer(
        pair.server.clone(),
        inspector,
        1234,
        &mut counts,
    ));
    let closed = timeout(Duration::from_secs(1), pair.client.closed()).await;
    pair.shutdown().await;
    assert!(
        closed.is_ok(),
        "an unpolled helper must release its accepted connection"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_while_waiting_for_authorization_ack_closes_the_connection() {
    let pair = Pair::new().await;
    let (_, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let outcome = timeout(Duration::from_secs(5), async {
        let held = {
            let pending = Session::from_peer(pair.client.clone(), pair.pin, &grant);
            tokio::pin!(pending);
            tokio::select! {
                _ = &mut pending => panic!("setup must wait for an authorization ack"),
                streams = pair.server.accept_bi() => streams.unwrap(),
            }
        }; // Cancel the polled setup; keep the remote streams alive.
        pair.server.closed().await;
        drop(held);
    })
    .await;
    pair.shutdown().await;
    outcome.expect("cancelled authorization close deadline");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_session_closes_only_its_peer_not_the_shared_endpoint() {
    let pair = Pair::new().await;
    let coordinator = node().await;
    let address = coordinator.local_addr().unwrap();
    pair.phone.connect(address).await.unwrap();
    let peer = coordinator.accept().await.unwrap();
    let receiver = coordinator
        .get_quic_connection(&peer.remote_addr.as_socket_addr().unwrap())
        .await
        .unwrap()
        .unwrap();
    let sender = pair
        .phone
        .get_quic_connection(&address)
        .await
        .unwrap()
        .unwrap();
    let (inspector, grant) = HttpInspector::new(1234).unwrap();
    let grant = Zeroizing::new(grant);
    let outcome = timeout(Duration::from_secs(5), async {
        let mut counts = Counts::default();
        let client = async {
            let session = Session::from_peer(pair.client.clone(), pair.pin, &grant)
                .await
                .unwrap();
            drop(session);
        };
        let (_, server) = tokio::join!(
            client,
            serve_peer(pair.server.clone(), inspector, 1234, &mut counts)
        );
        server.unwrap();
        assert!(counts.authorized);
        assert_eq!(counts.fetched, 0);
        let (mut send, _recv) = sender.open_bi().await.unwrap();
        send.write_all(b"coordinator alive").await.unwrap();
        send.finish().unwrap();
        let (_reply, mut recv) = receiver.accept_bi().await.unwrap();
        assert_eq!(recv.read_to_end(64).await.unwrap(), b"coordinator alive");
    })
    .await;
    pair.shutdown().await;
    coordinator.shutdown().await;
    outcome.expect("shared endpoint survives session drop");
}
