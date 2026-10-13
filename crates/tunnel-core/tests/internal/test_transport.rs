//! Test-only raw QUIC fixture for HTTP policy and wrong-ALPN cases.
//! Not a shipped listener, relay implementation or alternative application path.
use crate::{Failure, Result, pin};
use saorsa_transport::{
    ClientConfig, Endpoint, EndpointConfig, HighLevelConnection, ServerConfig, TransportConfig,
    crypto::{
        pqc::{PqcConfig, create_crypto_provider},
        raw_public_keys::{RawPublicKeyResolver, fingerprint_public_key, generate_ml_dsa_keypair},
        rustls::{QuicClientConfig, QuicServerConfig},
    },
    high_level::default_runtime,
};
use std::{
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Copy)]
pub(crate) enum WireMode {
    Application,
    Viewer,
}
impl WireMode {
    fn alpn(self) -> &'static [u8] {
        match self {
            Self::Application => b"web-p2p-tunnel.native-application.v1",
            Self::Viewer => b"web-p2p-tunnel.native-viewer.v1",
        }
    }
}
fn provider() -> Result<Arc<rustls::crypto::CryptoProvider>> {
    create_crypto_provider(&PqcConfig::default()).map_err(|_| Failure("pqc-provider"))
}
fn transport() -> Arc<TransportConfig> {
    let mut config = TransportConfig::default();
    config
        .max_concurrent_bidi_streams(33u32.into())
        .max_concurrent_uni_streams(0u32.into())
        .stream_receive_window((2 * 1024 * 1024u32).into())
        .receive_window((2 * 1024 * 1024u32).into())
        .send_window(2 * 1024 * 1024)
        .datagram_receive_buffer_size(None)
        .max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()))
        .keep_alive_interval(Some(Duration::from_secs(10)));
    Arc::new(config)
}
fn endpoint(address: SocketAddr, server: Option<ServerConfig>) -> Result<Endpoint> {
    let socket = UdpSocket::bind(address).map_err(|_| Failure("test-bind"))?;
    socket
        .set_nonblocking(true)
        .map_err(|_| Failure("test-socket"))?;
    Endpoint::new(
        EndpointConfig::default(),
        server,
        socket,
        default_runtime().ok_or(Failure("runtime"))?,
    )
    .map_err(|_| Failure("endpoint"))
}
pub(crate) async fn close(endpoint: &Endpoint) {
    endpoint.close(0u32.into(), b"fixture complete");
    let _ = tokio::time::timeout(Duration::from_secs(2), endpoint.wait_idle()).await;
}
pub(crate) struct Server {
    pub endpoint: Endpoint,
    pub public_pin: [u8; 32],
}
pub(crate) struct Fixture {
    pub backend: Server,
    pub destination: SocketAddr,
}
impl Fixture {
    pub async fn application_loopback() -> Result<Self> {
        let (public, secret) = generate_ml_dsa_keypair().map_err(|_| Failure("identity"))?;
        let public_pin = fingerprint_public_key(&public);
        let resolver =
            RawPublicKeyResolver::new(public, secret).map_err(|_| Failure("identity"))?;
        let mut tls = rustls::ServerConfig::builder_with_provider(provider()?)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|_| Failure("tls"))?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));
        tls.alpn_protocols = vec![WireMode::Application.alpn().to_vec()];
        let crypto = QuicServerConfig::try_from(tls).map_err(|_| Failure("tls"))?;
        let mut config = ServerConfig::with_crypto(Arc::new(crypto));
        config.transport_config(transport()).migration(false);
        let endpoint = endpoint("127.0.0.1:0".parse().unwrap(), Some(config))?;
        let destination = endpoint.local_addr().map_err(|_| Failure("test-address"))?;
        Ok(Self {
            backend: Server {
                endpoint,
                public_pin,
            },
            destination,
        })
    }
    pub async fn shutdown(&mut self) {
        close(&self.backend.endpoint).await;
    }
}
pub(crate) fn client_endpoint(
    local: SocketAddr,
    public_pin: [u8; 32],
    mode: WireMode,
) -> Result<(Endpoint, Arc<AtomicBool>)> {
    let rejected = Arc::new(AtomicBool::new(false));
    let verifier = pin::PinnedVerifier {
        pin: public_pin,
        rejected: rejected.clone(),
    };
    let mut tls = rustls::ClientConfig::builder_with_provider(provider()?)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| Failure("tls"))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    tls.alpn_protocols = vec![mode.alpn().to_vec()];
    tls.enable_early_data = false;
    tls.resumption = rustls::client::Resumption::disabled();
    let crypto = QuicClientConfig::try_from(tls).map_err(|_| Failure("tls"))?;
    let mut config = ClientConfig::new(Arc::new(crypto));
    config.transport_config(transport());
    let mut endpoint = endpoint(local, None)?;
    endpoint.set_default_client_config(config);
    Ok((endpoint, rejected))
}
pub(crate) async fn connect(
    endpoint: &Endpoint,
    remote: SocketAddr,
    rejected: &AtomicBool,
) -> Result<HighLevelConnection> {
    endpoint
        .connect(remote, "fixture.invalid")
        .map_err(|_| Failure("connect-start"))?
        .await
        .map_err(|_| {
            if rejected.load(Ordering::Relaxed) {
                Failure("pin-mismatch")
            } else {
                Failure("handshake")
            }
        })
}
