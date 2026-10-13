# Handshake ownership hooks: transport mechanism, application policy

## Why this seam is retained

The current client must refuse unsolicited work from endpoint construction, bound
helper admission and retain the exact authenticated connection. It cannot safely
recover those guarantees afterward by polling an address table or interpreting
an unclassified timeout as silence.

Patch 0005 on Saorsa 0.37.0 supplies a generic hook rather than embedding our routing,
queue size, deadline or one-shot grant policy in the transport. This is a local
reviewed extension, not an upstream-merged interface. It does not change TLS/QUIC
cryptography or MASQUE wire formats; remote participants need no adaptation.

## Generic interface

`PeerHandshakeHandler` can take ownership of ordinary incoming handshakes,
explicitly opt into virtual ingress, consume coordinator-requested outgoing dials,
and observe detachment. Returning an incoming/dial value preserves default
upstream dispatch. Taking ownership does not fabricate successful TLS results.
An owned incoming handshake is accepted with the existing server configuration;
`PeerDial::connect` uses the existing client crypto.

The transport supports constructor-installed handlers and removable registrations.
A registration identifies its installation, so a stale token cannot remove a newer
handler. Already transferred connections remain caller-owned after removal.
Virtual ingress defaults to pass-through without implicitly invoking the ordinary
incoming handler; the application must explicitly override it.

Callbacks are trusted, synchronous, thread-safe, non-blocking and non-panicking.
They execute outside registry locks. Removal does not cancel a callback already
selected for dispatch, so callers must own detachment and cleanup correctly.

## Current application policy

- [Phone ingress](../../crates/tunnel-core/src/phone_ingress.rs) permanently refuses
  ordinary/virtual incoming attempts and requested outgoing dials. Direct/Relay
  clients initiate their own connections. There is no arm state or coordinated
  attempt owner in the current implementation.
- The [helper](../../crates/tunnel-core/src/peer_route.rs) installs bounded admission
  at construction, limits pending TLS work and rejects requested outgoing dials.
  One TLS winner consumes the one-shot application authorization attempt.
- Completed connections still pass actual retained-peer pin verification before
  any grant-bearing application stream. Stop owns pending and transferred work.

See [constructor-time admission](saorsa-preparation-admission.md),
[virtual admission](saorsa-virtual-admission.md) and
[authorization/Stop ordering](saorsa-authorization-stop.md) for the motivating
findings. Historic test counts in those records are not current suite counts.

## Provenance

The applied file is
[0005-peer-handshake-hook.patch](../../patches/saorsa-transport/0005-peer-handshake-hook.patch),
SHA-256 `5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386`.
It applies after 0001–0004 on revision
`e2f08388088313c82cac393afa6ca27b9ce00516`. The superseded coordination-policy
prototype and its research are removed; they are not required build inputs.

The [upstream MIT notice](../../patches/saorsa-transport/UPSTREAM-LICENSE-MIT)
remains with the patch context. Original additions use the project's MIT license.
Use the [current build/test instructions](../BUILDING.md), not retired experiment
runners. No new public experiment or device validation is claimed by this summary.
