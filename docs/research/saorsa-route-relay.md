# Single-carrier Relay: ownership and withdrawal semantics

## Current connection path

The phone tries eligible Direct candidates, then a separately advertised Relay
contact. Only classified no-response permits fallback; TLS/pin/grant, input,
bootstrap, cancellation and cleanup errors are terminal. Coordination is not a
route in this implementation. See [candidate policy](../ROUTING-OFFER.md).

The phone sends ordinary UDP/QUIC to the relay allocation while the helper receives
inner QUIC through its MASQUE carrier. An ordinary phone socket does not make the
route Direct: offer provenance labels it Relay. The actual computer's pin is
checked on the retained inner connection before an application grant is sent.

## Helper ownership

The helper optionally requests one allocation from an explicitly configured
participant. It does not operate a relay or discover a global participant directory.
Bootstrap/allocation errors are terminal preparation errors, not an implicit
switch to a different participant. Clients disable relay serving, UPnP and
unwanted advertisements.

A Direct winner withdraws its unused relay allocation; a Relay winner retains the
allocation for the session. Stop, peer failure and helper completion drain owned
handshakes and close the allocation, endpoint and runtime. Local losing resources
must retire before the phone tries another route. Setup/cleanup deadlines are not
healthy-session lifetime limits. See [runtime ownership](saorsa-route-adapter-lifecycle.md)
and [active-stream teardown](saorsa-route-stream-teardown.md).

## Why withdrawal does not mean immediate remote socket reclamation

The original cleanup investigation found that an assertion requiring the relay's
UDP port to be immediately rebindable after abort was testing the wrong contract:

- Pinned [MasqueRelayConfig](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/masque/relay_server.rs#L294-L325)
  specifies stable-port reservations for authenticated peers. Defaults are a
  600-second freshness window and 1024-entry LRU cap. Expiry is lazy; the TTL does
  not promise timed socket release or impose an active-session lifetime.
- [close_session](https://github.com/WithAutonomi/saorsa-transport/blob/e2f08388088313c82cac393afa6ca27b9ce00516/src/masque/relay_server.rs#L1825-L1890)
  drains forwarding and removes the active session while retaining the bound
  socket in a peer-identity-keyed reservation for reconnect.

Application reachability withdrawal and server-owned reservation retention are
therefore different claims. The current client does not change that server policy
or promise immediate remote FD/quota reclamation. Local endpoint/runtime cleanup
is a separate application responsibility.

Current regressions live under
[core internal tests](../../crates/tunnel-core/tests/internal/), including Relay
selection, authorization, cancellation and stream teardown. Earlier three-route
experiment runners and test totals are not part of the current implementation.
See [release readiness](../RELEASE-READINESS.md) for validation boundaries.
