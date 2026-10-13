# Why the current client uses a patched Saorsa transport

The current Direct/Relay client pins Saorsa transport 0.37.0 at
`e2f08388088313c82cac393afa6ca27b9ce00516`. It inherits upstream QUIC/TLS,
cryptographic policy and MASQUE protocols. Existing relay participants do not
need our client-side patches. The application does not perform coordinated punching.

## Application versus transport responsibilities

The application owns route eligibility/order, retained-connection pin verification,
scoped grants, admission budgets and awaited cleanup. Upstream's catch-all fallback
cannot replace our rule that only classified no-response permits another route;
TLS/authentication/protocol errors must remain terminal. See the
[route-selection source review](saorsa-route-selection-reassessment.md).

Patch 0004 exposes an owned connecting attempt and exact accepted connections.
The adapter can abort a silent attempt without confusing started/stalled TLS with
no response, and can authenticate the connection it actually uses rather than
relooking up a potentially replaced address-table entry. See
[phase-aware attempts](saorsa-phase-aware-peer-attempts.md).

Patch 0005 supplies generic handshake ownership hooks, including constructor-time
installation and virtual ingress. The phone installs a permanently closed ingress
policy; the helper applies bounded admission from endpoint creation. Neither client
accepts coordinator-requested outgoing handshakes. These are application choices,
not global restrictions on ordinary upstream consumers. See
[the hook boundary](saorsa-handshake-hook-adapter.md).

## Current patch stack

- **0003:** Bounded client relay-response collection.
- **0004:** Caller-owned dialing, atomic abort classification and accepted handles.
- **0005:** Generic handshake ownership, virtual admission and constructor installation.

These three reviewed patches are applied with their original numbers and bytes.
Relay forwarding and socket construction use upstream's existing implementation;
no relay service is operated by this application. The
[current-scope audit](current-scope-patch-audit.md) records call-site evidence and
validation of this stack.

The authoritative hashes, application order, API limits and upstream MIT notice
are in the [patch stack](../../patches/saorsa-transport/README.md). Preparation
archives the exact upstream revision and patches a generated tree, never Cargo's
cached checkout. No discarded experiment patch is required.

These changes are not claimed upstream-merged or independently audited. Preserving
upstream cryptography does not eliminate application authorization, cancellation,
resource-limit or dependency-notice responsibilities. Current validation and
remaining binary/device gates are in [release readiness](../RELEASE-READINESS.md).
