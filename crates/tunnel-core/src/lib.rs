//! Shared Direct/Relay tunnel policy, sessions and owned transport lifecycle.
//! No JNI, terminal UI, diagnostics, alternate offers or public relay service.
pub mod application;
pub mod http;
pub mod peer_connect;
pub mod peer_route;
mod phone_ingress;
mod pin;
pub mod target;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure(pub &'static str);
pub type Result<T> = std::result::Result<T, Failure>;

#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/test_transport.rs"]
mod test_transport;
#[cfg(all(test, feature = "host"))]
use test_transport::{WireMode, client_endpoint, connect};
