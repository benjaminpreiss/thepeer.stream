//! Phase-aware application connection attempts. No automatic route fallback.
use crate::{Failure, Result, application::Session};
use saorsa_transport::P2pEndpoint;
use std::{net::SocketAddr, time::Duration};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

/// Some means full TLS, computer pin and app grant checks succeeded on one handle.
/// None permits another path ONLY after atomically aborting an attempt with no
/// recorded peer response or error; this does not prove the remote host is absent.
/// Every Err is terminal, including authentication and cancellation. Setup allows
/// 3s for a response, 10s for TLS, then the existing 10s application authorization.
/// Use an application-owned, client-only endpoint; its crypto/relay policy is unchanged.
/// The caller retains the endpoint. Stop/drop attempts and returned sessions BEFORE
/// shutting it down: upstream shutdown does not own these unregistered connections.
/// Successful sessions own their exact checked connection, never the shared
/// endpoint or grant storage. The cancellation token governs setup; after success,
/// the caller must also wire Stop into application::drive. No app data uses 0-RTT.
pub async fn open(
    endpoint: &P2pEndpoint,
    remote: SocketAddr,
    pin: [u8; 32],
    grant: &str,
    stop: &CancellationToken,
) -> Result<Option<Session>> {
    tokio::select! {
        biased;
        _ = stop.cancelled() => Err(Failure("peer-cancelled")),
        result = attempt(endpoint, remote, pin, grant) => result,
    }
}

async fn attempt(
    endpoint: &P2pEndpoint,
    remote: SocketAddr,
    pin: [u8; 32],
    grant: &str,
) -> Result<Option<Session>> {
    if grant.len() != 64
        || !grant
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Failure("peer-input"));
    }
    let mut connecting = endpoint
        .begin_connect(remote)
        .map_err(|_| Failure("peer-connect"))?;
    match timeout(Duration::from_secs(3), connecting.first_peer_response()).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return Err(Failure("peer-response")),
        Err(_) => {
            return match connecting.abort() {
                Ok(false) => Ok(None),
                Ok(true) => Err(Failure("peer-handshake-timeout")),
                Err(_) => Err(Failure("peer-response")),
            };
        }
    }
    let connection = match timeout(Duration::from_secs(10), &mut connecting).await {
        Ok(result) => result.map_err(|_| Failure("peer-handshake"))?,
        Err(_) => {
            let _ = connecting.abort();
            return Err(Failure("peer-handshake-timeout"));
        }
    };
    Session::from_peer(connection, pin, grant).await.map(Some)
}

#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/peer_connect_tests.rs"]
mod tests;
