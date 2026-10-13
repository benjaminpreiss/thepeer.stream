//! A configured HTTP authority, separate from the addresses used to connect.
use crate::{Failure, Result};
use std::{fmt, net::IpAddr, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    host: String,
    port: u16,
    authority: String,
}
impl Target {
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn authority(&self) -> &str {
        &self.authority
    }
}
#[cfg(feature = "host")]
#[derive(Clone)]
pub struct PreparedTarget {
    pub(crate) target: Target,
    addresses: std::sync::Arc<[std::net::SocketAddr]>,
}

#[cfg(feature = "host")]
impl Target {
    /// Resolve once before publishing an offer. The returned destination can
    /// only connect to the validated numeric addresses, never re-resolve DNS.
    pub async fn prepare(
        self,
        stop: &tokio_util::sync::CancellationToken,
    ) -> Result<PreparedTarget> {
        let host = self.host.clone();
        let port = self.port;
        let lookup = async move {
            if let Ok(ip) = host.parse::<IpAddr>() {
                return Ok(vec![std::net::SocketAddr::new(ip, port)]);
            }
            tokio::net::lookup_host((host.as_str(), port))
                .await
                .map(|addresses| addresses.take(17).collect::<Vec<_>>())
        };
        self.prepare_with(stop, lookup).await
    }

    pub(crate) async fn prepare_with(
        self,
        stop: &tokio_util::sync::CancellationToken,
        lookup: impl std::future::Future<Output = std::io::Result<Vec<std::net::SocketAddr>>>,
    ) -> Result<PreparedTarget> {
        let mut addresses = tokio::select! {
            biased;
            _ = stop.cancelled() => return Err(Failure("peer-cancelled")),
            result = tokio::time::timeout(std::time::Duration::from_secs(5), lookup) => {
                result.map_err(|_| Failure("target-resolution-timeout"))?
                    .map_err(|_| Failure("target-resolution"))?
            }
        };
        if addresses.is_empty() || addresses.len() > 16 || addresses.iter().any(|address| {
            !address.ip().is_loopback() || address.port() != self.port
                || matches!(address, std::net::SocketAddr::V6(v6) if v6.scope_id() != 0 || v6.flowinfo() != 0)
        }) {
            return Err(Failure("target-not-loopback"));
        }
        // Keep the resolver's preference, removing duplicates without reordering.
        let mut seen = std::collections::HashSet::new();
        addresses.retain(|address| seen.insert(*address));
        Ok(PreparedTarget {
            target: self,
            addresses: addresses.into(),
        })
    }
}

#[cfg(feature = "host")]
impl PreparedTarget {
    pub(crate) async fn connect(&self) -> std::io::Result<tokio::net::TcpStream> {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(self.addresses.as_ref()),
        )
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "target connect timeout"))?
    }

    #[cfg(test)]
    pub(crate) fn loopback(port: u16) -> Self {
        Self {
            target: format!("localhost:{port}").parse().unwrap(),
            addresses: vec![
                std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
                std::net::SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)),
            ]
            .into(),
        }
    }
}

#[cfg(all(test, feature = "host"))]
#[path = "../tests/internal/target_tests.rs"]
mod tests;

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.authority)
    }
}
impl FromStr for Target {
    type Err = Failure;
    fn from_str(text: &str) -> Result<Self> {
        let bad = || Failure("target-format");
        if text.len() > 260 || !text.is_ascii() {
            return Err(bad());
        }
        let (host, port) = text.rsplit_once(':').ok_or_else(bad)?;
        if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let port = port
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0)
            .ok_or_else(bad)?;
        let host = if host.starts_with('[') {
            let ip = host
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or_else(bad)?
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| bad())?;
            if !ip.is_loopback() {
                return Err(Failure("target-not-loopback"));
            }
            ip.to_string()
        } else if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
            if !ip.is_loopback() {
                return Err(Failure("target-not-loopback"));
            }
            ip.to_string()
        } else {
            let host = host.to_ascii_lowercase();
            // Strict ASCII DNS labels; no URLs, userinfo, scope IDs or
            // all-numeric shorthand addresses. Resolution still validates every IP.
            if host.is_empty()
                || host.len() > 253
                || host.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                || host.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || !label.as_bytes()[0].is_ascii_alphanumeric()
                        || !label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                        || !label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(bad());
            }
            host
        };
        let authority = match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => format!("[{host}]:{port}"),
            _ => format!("{host}:{port}"),
        };
        Ok(Self {
            host,
            port,
            authority,
        })
    }
}
