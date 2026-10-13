use super::*;
use saorsa_transport::high_level::{AsyncUdpSocket, UdpPoller};
use std::{
    io::{self, IoSliceMut},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::UdpSocket;

// Remote-only fault injection through the public socket interface. The genuine
// QUIC peer is told a false source address and consequently emits authenticated,
// encrypted observation frames for that address. All actual UDP sends remain loopback. No
// helper state, secret, crypto result or plaintext protocol frame is modified.
#[derive(Debug)]
struct ReportingSocket {
    socket: Arc<dyn AsyncUdpSocket>,
    physical_peer: SocketAddr,
    reported_peer: SocketAddr,
}
impl AsyncUdpSocket for ReportingSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.socket.clone().create_io_poller()
    }
    fn try_send(&self, transmit: &quinn_udp::Transmit<'_>) -> io::Result<()> {
        assert_eq!(transmit.destination, self.reported_peer);
        let mut transmit = transmit.clone();
        transmit.destination = self.physical_peer;
        self.socket.try_send(&transmit)
    }
    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [quinn_udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        match self.socket.poll_recv(cx, bufs, meta) {
            Poll::Ready(Ok(count)) => {
                for meta in &mut meta[..count] {
                    assert_eq!(meta.addr, self.physical_peer);
                    meta.addr = self.reported_peer;
                }
                Poll::Ready(Ok(count))
            }
            result => result,
        }
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }
    fn max_transmit_segments(&self) -> usize {
        self.socket.max_transmit_segments()
    }
    fn max_receive_segments(&self) -> usize {
        self.socket.max_receive_segments()
    }
    fn may_fragment(&self) -> bool {
        self.socket.may_fragment()
    }
}

fn observer(physical_peer: SocketAddr, reported_peer: SocketAddr) -> saorsa_transport::Endpoint {
    use saorsa_transport::{
        Endpoint, EndpointConfig, ServerConfig, TransportConfig,
        crypto::{
            pqc::PqcConfig,
            raw_public_keys::{RawPublicKeyConfigBuilder, key_utils::generate_ml_dsa_keypair},
            rustls::QuicServerConfig,
        },
    };
    let (public, secret) = generate_ml_dsa_keypair().unwrap();
    let tls = RawPublicKeyConfigBuilder::new()
        .with_server_key(public, secret)
        .allow_any_key()
        .with_pqc(PqcConfig::default())
        .build_rfc7250_server_config()
        .unwrap();
    let crypto = QuicServerConfig::try_from(tls.inner().as_ref().clone()).unwrap();
    let mut config = ServerConfig::with_crypto(Arc::new(crypto));
    let mut transport = TransportConfig::default();
    transport.enable_address_discovery(true);
    config.transport_config(Arc::new(transport));
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    let runtime = saorsa_transport::high_level::default_runtime().unwrap();
    let socket = Arc::new(ReportingSocket {
        socket: runtime.wrap_udp_socket(socket).unwrap(),
        physical_peer,
        reported_peer,
    });
    Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(config), socket, runtime)
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn authenticated_ineligible_observation_returns_no_offer_or_target_access() {
    timeout(Duration::from_secs(45), async {
        for (relay_only, report) in [false, true].into_iter().flat_map(|relay_only| {
            [
                None,
                Some("127.0.0.1:0"),
                Some("0.0.0.0:12345"),
                Some("224.0.0.1:12345"),
                Some("255.255.255.255:12345"),
                Some("[::]:12345"),
                Some("[ff02::1]:12345"),
                Some("[fe80::1]:12345"),
            ]
            .into_iter()
            .filter(move |report| !relay_only || report.is_some())
            .map(move |report| (relay_only, report))
        }) {
            let reservation = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let bind = reservation.local_addr().unwrap();
            let reported = report.map(|value| value.parse().unwrap()).unwrap_or(bind);
            let remote = observer(bind, reported);
            let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let stop = CancellationToken::new();
            let config = HelperConfig {
                bind,
                target_port: target.local_addr().unwrap().port(),
                observer: (!relay_only).then_some(remote.local_addr().unwrap()),
            };
            let participant = remote.local_addr().unwrap();
            drop(reservation);
            let preparing_stop = stop.clone();
            let preparing = tokio::spawn(async move {
                if relay_only {
                    Helper::prepare_with_relay(config, participant, &preparing_stop).await
                } else {
                    Helper::prepare(config, &preparing_stop).await
                }
            });
            let control = remote.accept().await.unwrap().await.unwrap();
            assert_eq!(control.remote_address(), reported);
            let result = preparing.await.unwrap();
            if report.is_none() {
                let (helper, offer) = result.unwrap();
                assert_eq!(offer.direct_candidate(), bind);
                helper.shutdown().await.unwrap();
            } else {
                assert_eq!(
                    result.as_ref().err().map(|error| error.0),
                    Some("route-observed-address"),
                    "authenticated report {reported} is untrusted input (relay-only={relay_only})"
                );
            }
            assert!(
                control.stats().frame_tx.observed_address > 0,
                "real observation frame sent"
            );
            timeout(Duration::from_secs(3), control.closed())
                .await
                .unwrap();
            let _released = UdpSocket::bind(bind).await.unwrap();
            assert!(!stop.is_cancelled());
            assert!(
                timeout(Duration::from_millis(100), target.accept())
                    .await
                    .is_err()
            );
            if relay_only {
                assert!(
                    timeout(Duration::from_millis(100), remote.accept())
                        .await
                        .is_err(),
                    "invalid observation must not start an allocation carrier"
                );
            }
            remote.close(0u32.into(), b"observer fixture finished");
            timeout(Duration::from_secs(3), remote.wait_idle())
                .await
                .unwrap();
        }
    })
    .await
    .expect("bounded authenticated observer fixture");
}
