//! Application routing/lifecycle adapter. Policy stays outside the transport.
//! Production: eligible Direct candidates → available single-carrier relay.
#[cfg(feature = "host")]
use crate::http::{
    host::{Counts, Progress},
    inspector::HttpInspector,
};
use crate::{Failure, Result, application};
use saorsa_transport::{
    NatConfig, P2pConfig, P2pEndpoint, peer_handshake::PeerHandshakeHandler, upnp::UpnpConfig,
};
#[cfg(feature = "host")]
use saorsa_transport::{
    crypto::{pqc::types::MlDsaPublicKey, raw_public_keys::fingerprint_public_key},
    high_level::{Connecting, Connection, Incoming},
    nat_traversal_api::PreparedRelay,
    peer_handshake::PeerDial,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle, time::timeout};
#[cfg(feature = "host")]
use tokio::{sync::mpsc, task::JoinSet};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use zeroize::Zeroizing;
#[path = "route_offer.rs"]
mod offer_codec;
#[path = "route_plan.rs"]
mod planner;
#[path = "route_runtime.rs"]
mod runtime;

/// Bind must be explicit unicast (port 0 allowed), with no scope/flow metadata
/// or mapped/link-local IPv6. Callers should default to loopback. All requested
/// participants must match its family and are validated before endpoint creation,
/// not just by the host CLI.
#[cfg(feature = "host")]
pub struct HelperConfig {
    pub bind: SocketAddr,
    pub target_port: u16,
    /// Explicit participant. Bootstrap and acquisition of its ordinary-UDP
    /// observation are required; failure is terminal, not silently skipped.
    pub observer: Option<SocketAddr>,
}
/// Owned capability, deliberately neither Debug nor Clone nor generic Serialize.
/// Only the explicit target offer codec may transfer it; never log or persist it.
pub struct Offer {
    direct: SocketAddr,
    // Additive wpa2 hints: retain the actual bind alongside its observation.
    bound: Option<SocketAddr>,
    observer: Option<SocketAddr>,
    pin: [u8; 32],
    grant: Zeroizing<String>,
    relay: Option<SocketAddr>,
}
impl Offer {
    /// Routing hints are not identity. Pin and grant remain opaque and unchanged.
    pub fn direct_candidate(&self) -> SocketAddr {
        self.direct
    }
    /// Public routing contact, not the grant or a relay serving capability.
    pub fn relay_candidate(&self) -> Option<SocketAddr> {
        self.relay
    }
    pub fn set_direct_candidate(&mut self, address: SocketAddr) {
        self.direct = address;
        if self.observer.is_none() {
            self.bound = Some(address);
        }
    }
}
#[cfg(feature = "host")]
pub struct Helper {
    progress: tokio::sync::watch::Receiver<Progress>,
    stop: CancellationToken,
    task: Option<JoinHandle<Result<()>>>,
    runtime: Option<runtime::Owner>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Direct,
    Relay,
}
/// Owns a private transport runtime. Drop closes the peer, signals endpoint
/// cleanup, then ends that runtime. Explicit serve completion waits for teardown.
pub struct Connected {
    // Declaration order matters for an unpolled serve future: peer before endpoint.
    session: application::Session,
    endpoint: EndpointOwner,
    setup_stop: CancellationToken,
    runtime: Option<runtime::Owner>,
    route: Route,
}

// An already-running supervisor, not a best-effort spawn from a destructor.
// Dropping the sender requests shutdown; finish additionally waits for completion.
struct EndpointOwner {
    endpoint: P2pEndpoint,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<()>>,
}
impl EndpointOwner {
    async fn create_with_handler(
        bind: SocketAddr,
        cleanup: &TaskTracker,
        handler: Option<Arc<dyn PeerHandshakeHandler>>,
    ) -> Result<Self> {
        let config = P2pConfig::builder()
            .bind_addr(bind)
            .max_connections(12)
            .nat(NatConfig {
                allow_loopback: true,
                enable_relay_service: false,
                advertise_external_addresses: false,
                upnp: UpnpConfig::disabled(),
                ..NatConfig::default()
            })
            .build()
            .map_err(|_| Failure("route-config"))?;
        let endpoint = match handler {
            Some(handler) => P2pEndpoint::new_with_handshake_handler(config, handler)
                .await
                .map(|(endpoint, _registration)| endpoint),
            None => P2pEndpoint::new(config).await,
        }
        .map_err(|_| Failure("route-endpoint"))?;
        let (stop, stopped) = oneshot::channel();
        let owned = endpoint.clone();
        let task = cleanup.spawn(async move {
            let _ = stopped.await;
            timeout(Duration::from_secs(8), owned.shutdown())
                .await
                .map_err(|_| Failure("route-shutdown"))
        });
        Ok(Self {
            endpoint,
            stop: Some(stop),
            task,
        })
    }
    async fn finish(mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        (&mut self.task)
            .await
            .map_err(|_| Failure("route-cleanup-task"))?
    }
}

#[cfg(feature = "host")]
enum Pending {
    Incoming(Incoming),
}
#[cfg(feature = "host")]
impl Pending {
    fn refuse(self) {
        let Self::Incoming(incoming) = self;
        incoming.refuse();
    }
    fn begin(self) -> Result<Connecting> {
        match self {
            Self::Incoming(incoming) => incoming.accept().map_err(|_| Failure("route-handshake")),
        }
    }
}
#[cfg(feature = "host")]
struct Inbox {
    tx: mpsc::Sender<Pending>,
    ready: std::sync::atomic::AtomicBool,
}
#[cfg(feature = "host")]
impl PeerHandshakeHandler for Inbox {
    fn incoming_virtual(&self, incoming: Incoming) -> Option<Incoming> {
        self.incoming(incoming)
    }
    fn incoming(&self, incoming: Incoming) -> Option<Incoming> {
        if !self.ready.load(std::sync::atomic::Ordering::Acquire) {
            incoming.refuse();
            return None;
        }
        if let Err(error) = self.tx.try_send(Pending::Incoming(incoming)) {
            error.into_inner().refuse();
        }
        None
    }
    fn outgoing(&self, _dial: PeerDial) -> Option<PeerDial> {
        // This helper never initiates coordinator-requested peer handshakes.
        None
    }
}
// A completed but unselected task result must also close its connection on Drop.
#[cfg(feature = "host")]
struct ReadyPeer(Option<Connection>);
#[cfg(feature = "host")]
impl Drop for ReadyPeer {
    fn drop(&mut self) {
        if let Some(peer) = &self.0 {
            peer.close(0u32.into(), b"unused helper peer");
        }
    }
}
#[cfg(feature = "host")]
async fn next_peer(
    rx: &mut mpsc::Receiver<Pending>,
    pending: &mut JoinSet<Option<ReadyPeer>>,
) -> Result<ReadyPeer> {
    loop {
        tokio::select! {
            result = pending.join_next(), if !pending.is_empty() => {
                match result {
                    Some(Ok(Some(peer))) => return Ok(peer),
                    // Listener admission, not phone fallback classification. No
                    // inspector/grant is consumed by rejected or expired TLS work.
                    Some(Ok(None)) => {},
                    _ => return Err(Failure("route-helper-task")),
                }
            }
            incoming = rx.recv(), if pending.len() < 4 => {
                let incoming = incoming.ok_or(Failure("route-listener"))?;
                pending.spawn(async move {
                    let connecting = incoming.begin().ok()?;
                    timeout(Duration::from_secs(10), connecting).await.ok()?.ok()
                        .map(|peer| ReadyPeer(Some(peer)))
                });
            }
        }
    }
}
#[cfg(feature = "host")]
async fn abort_allocation(
    endpoint: &P2pEndpoint,
    allocation: &mut Option<PreparedRelay>,
) -> Result<()> {
    if let Some(prepared) = *allocation {
        timeout(
            Duration::from_secs(5),
            endpoint.abort_proactive_relay(prepared),
        )
        .await
        .map_err(|_| Failure("route-relay-abort-timeout"))?
        .map_err(|_| Failure("route-relay-abort"))?;
        *allocation = None;
    }
    Ok(())
}
fn valid_candidate(address: SocketAddr) -> bool {
    planner::valid_address(address)
}
async fn bootstrap(
    endpoint: &P2pEndpoint,
    participant: SocketAddr,
    stop: &CancellationToken,
) -> Result<()> {
    tokio::select! {
        biased;
        _ = stop.cancelled() => Err(Failure("peer-cancelled")),
        result = timeout(Duration::from_secs(12), endpoint.connect(participant)) => {
            result.map_err(|_| Failure("route-bootstrap-timeout"))?
                .map_err(|_| Failure("route-bootstrap"))?;
            Ok(())
        }
    }
}

// Observations belong to a specific ordinary-UDP control connection. The
// endpoint-wide snapshot can come from another peer, and None can mean lock
// contention or a frame still in flight rather than absence of a mapping.
async fn observed_candidate(
    endpoint: &P2pEndpoint,
    participant: SocketAddr,
    stop: &CancellationToken,
) -> Result<SocketAddr> {
    tokio::select! {
        biased;
        _ = stop.cancelled() => Err(Failure("peer-cancelled")),
        result = timeout(Duration::from_secs(5), async {
            let control = endpoint.get_quic_connection(&participant).await
                .map_err(|_| Failure("route-observer-control"))?
                .ok_or(Failure("route-observer-control"))?;
            if control.is_virtual_socket() {
                return Err(Failure("route-observer-provenance"));
            }
            let mut tick = tokio::time::interval(Duration::from_millis(20));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = control.closed() => return Err(Failure("route-observer-control")),
                    _ = tick.tick() => {
                        if let Some(observed) = control.observed_address() {
                            // Transport may report IPv4 through a dual-stack socket.
                            // Canonicalize that representation before encoding a hint.
                            let observed = SocketAddr::new(observed.ip().to_canonical(), observed.port());
                            if !valid_candidate(observed) {
                                return Err(Failure("route-observed-address"));
                            }
                            if control.close_reason().is_some() {
                                return Err(Failure("route-observer-control"));
                            }
                            return Ok(observed);
                        }
                    }
                }
            }
        }) => result.map_err(|_| Failure("route-observation-timeout"))?,
    }
}

#[cfg(feature = "host")]
impl Helper {
    /// Request one allocation from an explicit existing participant. Failure is
    /// terminal preparation failure, not permission to silently remove a route.
    /// Without a separate observer, the relay's ordinary-UDP observation supplies
    /// Direct before allocation. The allocation remains a distinct Relay contact.
    pub async fn prepare_with_relay(
        config: HelperConfig,
        relay: SocketAddr,
        stop: &CancellationToken,
    ) -> Result<(Self, Offer)> {
        Self::prepare_routes(config, Some(relay), stop).await
    }
    /// Generates identity/grant locally and starts owning the listener before
    /// returning the offer. An explicit observer is bootstrapped, then its
    /// observation supplies the primary Direct hint alongside the bound socket.
    /// Without a participant,
    /// the bind address remains the direct hint. Neither guarantees reachability.
    /// This entry does not allocate a relay; prepare_with_relay requests one.
    pub async fn prepare(config: HelperConfig, stop: &CancellationToken) -> Result<(Self, Offer)> {
        Self::prepare_routes(config, None, stop).await
    }
    async fn prepare_routes(
        config: HelperConfig,
        relay: Option<SocketAddr>,
        stop: &CancellationToken,
    ) -> Result<(Self, Offer)> {
        if stop.is_cancelled() {
            return Err(Failure("peer-cancelled"));
        }
        // Bind may request an ephemeral port, but its address must be explicit
        // and representable. Validate every requested contact before creating an
        // endpoint or contacting even a different, otherwise valid participant.
        let mut bind_candidate = config.bind;
        bind_candidate.set_port(1);
        if !valid_candidate(bind_candidate)
            || config
                .observer
                .is_some_and(|address| !valid_candidate(address))
            || relay.is_some_and(|address| !valid_candidate(address))
        {
            return Err(Failure("route-input"));
        }
        if config
            .observer
            .into_iter()
            .chain(relay)
            .any(|address| address.is_ipv4() != config.bind.is_ipv4())
        {
            return Err(Failure("route-family"));
        }
        let stop = stop.clone();
        let ((mut helper, offer), runtime) = runtime::run(move |cleanup| async move {
            Self::prepare_owned(config, relay, &stop, cleanup).await
        })
        .await?;
        helper.runtime = Some(runtime);
        Ok((helper, offer))
    }
    async fn prepare_owned(
        config: HelperConfig,
        relay: Option<SocketAddr>,
        stop: &CancellationToken,
        cleanup: TaskTracker,
    ) -> Result<(Self, Offer)> {
        if stop.is_cancelled() {
            return Err(Failure("peer-cancelled"));
        }
        let (inspector, grant) =
            HttpInspector::new(config.target_port).map_err(|_| Failure("route-target"))?;
        let grant = Zeroizing::new(grant);
        // Reject unsolicited ingress and coordination instructions from the first
        // accept-loop poll, including construction and bootstrap. Our explicit
        // outgoing observer/carrier connections retain normal transport handling.
        let (tx, mut rx) = mpsc::channel(4);
        let inbox = Arc::new(Inbox {
            tx,
            ready: std::sync::atomic::AtomicBool::new(false),
        });
        let endpoint =
            EndpointOwner::create_with_handler(config.bind, &cleanup, Some(inbox.clone())).await?;
        let pin = fingerprint_public_key(
            &MlDsaPublicKey::from_bytes(endpoint.endpoint.public_key_bytes())
                .map_err(|_| Failure("route-identity"))?,
        );
        let bound = endpoint
            .endpoint
            .local_addr()
            .ok_or(Failure("route-address"))?;
        let mut direct = if let Some(observer) = config.observer {
            bootstrap(&endpoint.endpoint, observer, stop).await?;
            observed_candidate(&endpoint.endpoint, observer, stop).await?
        } else {
            bound
        };
        let mut allocation = if let Some(relay) = relay {
            bootstrap(&endpoint.endpoint, relay, stop).await?;
            if config.observer.is_none() {
                // Observe the original UDP socket before acquiring the separate
                // carrier/allocation. This does not opt into coordination.
                direct = observed_candidate(&endpoint.endpoint, relay, stop).await?;
            }
            let prepared = tokio::select! {
                biased;
                _ = stop.cancelled() => return Err(Failure("peer-cancelled")),
                result = timeout(Duration::from_secs(15), endpoint.endpoint.prepare_proactive_relay(relay)) => {
                    result.map_err(|_| Failure("route-relay-allocation-timeout"))?
                        .map_err(|_| Failure("route-relay-allocation"))?
                }
            };
            let address = prepared.public_addr();
            if address.port() == 0 || address.ip().to_canonical() != relay.ip().to_canonical() {
                return Err(Failure("route-relay-address"));
            }
            Some(prepared)
        } else {
            None
        };
        let relay_contact = allocation.map(|prepared| prepared.public_addr());
        let owned_stop = stop.child_token();
        let task_stop = owned_stop.clone();
        let (mut counts, progress) = Counts::monitored();
        // One-way transition to the same bounded inbox, never default dispatch.
        inbox
            .ready
            .store(true, std::sync::atomic::Ordering::Release);
        let task = tokio::spawn(async move {
            let mut pending = JoinSet::new();
            let result = tokio::select! {
                biased;
                _ = task_stop.cancelled() => Ok(()),
                result = async {
                    let mut selected = next_peer(&mut rx, &mut pending).await?;
                    if selected.0.as_ref().unwrap().is_virtual_socket() && allocation.is_none() {
                        return Err(Failure("route-relay-provenance"));
                    }
                    // One TLS winner receives the UNIQUE inspector. No copies of
                    // grant authorization are given to competing connections.
                    rx.close();
                    while let Ok(incoming) = rx.try_recv() { incoming.refuse(); }
                    pending.shutdown().await;
                    if !selected.0.as_ref().unwrap().is_virtual_socket() {
                        abort_allocation(&endpoint.endpoint, &mut allocation).await?;
                    }
                    application::serve_peer(selected.0.take().unwrap(), inspector, config.target_port, &mut counts).await
                } => result,
            };
            rx.close();
            while let Ok(incoming) = rx.try_recv() {
                incoming.refuse();
            }
            drop(rx);
            pending.shutdown().await;
            let relay_cleanup = abort_allocation(&endpoint.endpoint, &mut allocation).await;
            let cleanup = endpoint.finish().await;
            result.and(relay_cleanup).and(cleanup)
        });
        Ok((
            Self {
                progress,
                stop: owned_stop,
                task: Some(task),
                runtime: None,
            },
            Offer {
                direct,
                bound: Some(bound),
                observer: config.observer.or(relay),
                pin,
                grant,
                relay: relay_contact,
            },
        ))
    }
    /// Non-secret presentation state. Admission is not authorization or HTTP success.
    pub fn progress(&self) -> Progress {
        *self.progress.borrow()
    }
    /// Wait for session completion, without imposing a session lifetime. Cancelling
    /// this wait leaves the task owned and waitable. After completion, call shutdown
    /// to finish the private runtime; the session result is returned only here.
    pub async fn wait(&mut self) -> Result<()> {
        let result = self
            .task
            .as_mut()
            .ok_or(Failure("route-helper-task"))?
            .await
            .map_err(|_| Failure("route-helper-task"))
            .and_then(|result| result);
        self.task.take();
        result
    }
    /// Stop this helper (not its parent's token) and await owned cleanup.
    pub async fn shutdown(mut self) -> Result<()> {
        self.stop.cancel();
        let result = if self.task.is_some() {
            self.wait().await
        } else {
            Ok(())
        };
        let cleanup = match self.runtime.take() {
            Some(runtime) => runtime.finish().await,
            None => Ok(()),
        };
        result.and(cleanup)
    }
}
#[cfg(feature = "host")]
impl Drop for Helper {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

/// Production Direct/Relay path. A phone observation supplies only a shared-WAN
/// hint; no grant goes to the observer. Its private runtime is retired before
/// dialing application candidates. Each losing candidate is likewise fully
/// retired before the next, and only classified no-response permits fallback.
/// Explicit observer bootstrap/observation/cleanup failures are terminal.
pub async fn connect(
    mut offer: Offer,
    bind: SocketAddr,
    stop: &CancellationToken,
) -> Result<Connected> {
    if stop.is_cancelled() {
        return Err(Failure("peer-cancelled"));
    }
    // Validate even unused hints before any observer traffic.
    if !valid_candidate(offer.direct)
        || offer.bound.is_some_and(|a| !valid_candidate(a))
        || offer.observer.is_some_and(|a| !valid_candidate(a))
        || offer.relay.is_some_and(|a| !valid_candidate(a))
    {
        return Err(Failure("route-input"));
    }
    let own_observation = if let Some(observer) = offer.observer {
        if observer.is_ipv4() != bind.is_ipv4() {
            return Err(Failure("route-family"));
        }
        let owned_stop = stop.clone();
        let (address, owner) = runtime::run(move |cleanup| async move {
            let ingress = crate::phone_ingress::PhoneIngress::new();
            let endpoint =
                EndpointOwner::create_with_handler(bind, &cleanup, Some(ingress)).await?;
            let result = async {
                bootstrap(&endpoint.endpoint, observer, &owned_stop).await?;
                observed_candidate(&endpoint.endpoint, observer, &owned_stop).await
            }
            .await;
            let cleanup = endpoint.finish().await;
            match result {
                Err(error) => Err(error),
                Ok(address) => cleanup.map(|()| address),
            }
        })
        .await?;
        owner.finish().await?;
        Some(address)
    } else {
        None
    };
    let plan = planner::plan(
        planner::Candidates {
            bound: offer.bound.unwrap_or(offer.direct),
            observed: offer.observer.map(|_| offer.direct),
            relay: offer.relay,
        },
        own_observation,
    )
    .map_err(|_| Failure("route-input"))?;
    for attempt in plan.attempts() {
        if stop.is_cancelled() {
            return Err(Failure("peer-cancelled"));
        }
        let (route, address) = match attempt {
            planner::Attempt::Direct(address) => (Route::Direct, address),
            planner::Attempt::Relay(address) => (Route::Relay, address),
        };
        if address.is_ipv4() != bind.is_ipv4() {
            return Err(Failure("route-family"));
        }
        if route == Route::Direct {
            offer.direct = address;
        }
        let owned_stop = stop.clone();
        let ((returned, session), owner) = runtime::run(move |cleanup| async move {
            connect_owned(offer, bind, &owned_stop, route, cleanup).await
        })
        .await?;
        offer = returned;
        if let Some(mut connected) = session {
            connected.runtime = Some(owner);
            return Ok(connected);
        }
        owner.finish().await?;
    }
    Err(Failure("route-unavailable"))
}

async fn connect_owned(
    offer: Offer,
    bind: SocketAddr,
    stop: &CancellationToken,
    route: Route,
    cleanup: TaskTracker,
) -> Result<(Offer, Option<Connected>)> {
    if stop.is_cancelled() {
        return Err(Failure("peer-cancelled"));
    }
    let ingress = crate::phone_ingress::PhoneIngress::new();
    let endpoint =
        EndpointOwner::create_with_handler(bind, &cleanup, Some(ingress.clone())).await?;
    let result = match route {
        Route::Direct => {
            crate::peer_connect::open(
                &endpoint.endpoint,
                offer.direct,
                offer.pin,
                &offer.grant,
                stop,
            )
            .await
        }
        Route::Relay => {
            let address = offer.relay.ok_or(Failure("route-input"))?;
            crate::peer_connect::open(&endpoint.endpoint, address, offer.pin, &offer.grant, stop)
                .await
        }
    };
    match result {
        Ok(Some(session)) => Ok((
            offer,
            Some(Connected {
                session,
                endpoint,
                setup_stop: stop.clone(),
                runtime: None,
                route,
            }),
        )),
        result => {
            let cleanup = endpoint.finish().await;
            match result {
                Err(error) => Err(error),
                _ => {
                    cleanup?;
                    Ok((offer, None))
                }
            }
        }
    }
}
impl Connected {
    pub fn route(&self) -> Route {
        self.route
    }
    /// The setup token also stops the healthy session. No automatic reconnect or
    /// healthy-session deadline. Explicit return waits for endpoint cleanup.
    pub async fn serve(
        self,
        listener: TcpListener,
        hostname: String,
        stop: &CancellationToken,
    ) -> Result<()> {
        let result = application::drive(listener, self.session, hostname, async {
            tokio::select! { _ = stop.cancelled() => {}, _ = self.setup_stop.cancelled() => {} }
        })
        .await
        .map(|_| ());
        let cleanup = self.endpoint.finish().await;
        let runtime_cleanup = match self.runtime {
            Some(runtime) => runtime.finish().await,
            None => Ok(()),
        };
        result.and(cleanup).and(runtime_cleanup)
    }
}
#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/routing.rs"]
mod tests;
