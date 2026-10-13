// Copyright 2024 Saorsa Labs Limited
//
// The local-address classification and contextual planning approach are adapted
// from saorsa-core at 75bb32bdb3476afe17d7f5248f27a8a216a7f326:
// src/address.rs and src/dht_network_manager.rs. Upstream is licensed under
// MIT OR Apache-2.0. This adaptation uses the MIT option; see
// route-plan-LICENSE-MIT. Application-specific changes: bounded invitation
// inputs, Direct before Relay, no discovery, racing or coordinated punching.

//! Pure candidate planning, not a dialer or a reachability/security decision.
//!
//! The caller supplies observations from retained ordinary UDP connections, NOT
//! allocated relay contacts or endpoint-global snapshots. Nothing here discovers
//! addresses, opens sockets, selects an address family, authorizes a peer, or
//! permits fallback after an error. The execution adapter must still do all
//! validation/authorization and Stop/cleanup work appropriate to each attempt.
//!
//! Used by the Direct/Relay application path; the older three-route host review
//! entry point is deliberately separate.

use std::net::{IpAddr, SocketAddr};

/// At most two Direct addresses and one separately tagged relay contact.
#[derive(Clone, Copy)]
pub struct Candidates {
    /// Helper's actual bound socket. May be local-scope or globally addressed.
    pub bound: SocketAddr,
    /// Observation of that SAME helper UDP endpoint, not its relay allocation.
    pub observed: Option<SocketAddr>,
    /// Computer-owned MASQUE allocation, not the relay's service/control address.
    pub relay: Option<SocketAddr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attempt {
    Direct(SocketAddr),
    Relay(SocketAddr),
}

/// Fixed-size plan: at least one and at most three entries. No capabilities.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan([Option<Attempt>; 3]);

impl Plan {
    pub fn attempts(&self) -> impl Iterator<Item = Attempt> + '_ {
        self.0.iter().copied().flatten()
    }
}

/// Invalid input is terminal, including an invalid unused candidate/observation.
#[derive(Debug, PartialEq, Eq)]
pub struct InvalidAddress;

/// Plan eligible local Direct, other Direct, then Relay; no asynchronous work.
///
/// `own_observation` describes the PHONE's ordinary UDP endpoint. Comparing its
/// non-local IP to the helper's observed non-local IP gives only a shared-WAN
/// hint. Ports need not match. CGNAT/segmentation can give false positives; the
/// actual retained connection must still pass the computer pin before any grant.
///
/// Following core's conservative contextual rule, local-scope candidates are
/// included when that hint matches, or when no non-local Direct/Relay alternative
/// exists. An absent observation never means "same LAN". In particular this
/// function cannot fix LAN selection until the caller supplies suitable context.
///
/// Unlike core's general network ordering, eligible Direct always precedes Relay.
/// Identical Direct socket addresses are deduplicated; relay provenance is never
/// merged with Direct, even if the numeric socket address happens to match.
pub fn plan(
    candidates: Candidates,
    own_observation: Option<SocketAddr>,
) -> Result<Plan, InvalidAddress> {
    let Candidates {
        bound,
        observed,
        relay,
    } = candidates;
    for address in [Some(bound), observed, relay, own_observation]
        .into_iter()
        .flatten()
    {
        if !valid_address(address) {
            return Err(InvalidAddress);
        }
    }

    let shares_wan = own_observation.zip(observed).is_some_and(|(own, peer)| {
        !is_local(own.ip()) && !is_local(peer.ip()) && own.ip() == peer.ip()
    });
    let has_non_local_direct = [Some(bound), observed]
        .into_iter()
        .flatten()
        .any(|address| !is_local(address.ip()));
    let allow_local = shares_wan || (!has_non_local_direct && relay.is_none());

    let mut entries = [None; 3];
    let mut len = 0;
    // Keep locality preference explicit rather than relying on field order.
    for local in [true, false] {
        for address in [Some(bound), observed].into_iter().flatten() {
            if is_local(address.ip()) != local || (local && !allow_local) {
                continue;
            }
            let attempt = Attempt::Direct(address);
            if !entries[..len].contains(&Some(attempt)) {
                entries[len] = Some(attempt);
                len += 1;
            }
        }
    }
    if let Some(address) = relay {
        entries[len] = Some(Attempt::Relay(address));
    }
    Ok(Plan(entries))
}

// Shared with the application's codec and dial input validation.
pub(crate) fn valid_address(address: SocketAddr) -> bool {
    let ip = address.ip();
    address.port() != 0
        && !matches!(address, SocketAddr::V6(v6)
            if v6.scope_id() != 0 || v6.flowinfo() != 0
                || v6.ip().to_ipv4_mapped().is_some() || v6.ip().is_unicast_link_local())
        && !ip.is_unspecified()
        && !ip.is_multicast()
        && !matches!(ip, IpAddr::V4(ip) if ip.is_broadcast())
}

// Adapted from core's is_lan_ip: includes CGNAT, not just RFC1918. "Not local"
// does not mean globally reachable; this is classification, not a dial-back probe.
fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || (ip.octets()[0] == 100 && (ip.octets()[1] & 0b1100_0000) == 64)
        }
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

#[cfg(test)]
#[path = "../tests/internal/route_plan_tests.rs"]
mod tests;
