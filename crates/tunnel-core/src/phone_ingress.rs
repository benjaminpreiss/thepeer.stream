//! Constructor-installed, permanently closed unsolicited-ingress policy.
//! Direct/Relay clients initiate their own connections. There is no arm state,
//! coordinated dial, or handler-removal/reinstallation window.
use saorsa_transport::{
    high_level::Incoming,
    peer_handshake::{PeerDial, PeerHandshakeHandler},
};
use std::sync::Arc;

pub(crate) struct PhoneIngress;
impl PhoneIngress {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}
impl PeerHandshakeHandler for PhoneIngress {
    fn incoming(&self, incoming: Incoming) -> Option<Incoming> {
        incoming.refuse();
        None
    }
    fn incoming_virtual(&self, incoming: Incoming) -> Option<Incoming> {
        incoming.refuse();
        None
    }
    fn outgoing(&self, _dial: PeerDial) -> Option<PeerDial> {
        None
    }
}
