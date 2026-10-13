# Reviewed Saorsa transport patch stack

Base: **0.37.0**, `WithAutonomi/saorsa-transport` revision
`e2f08388088313c82cac393afa6ca27b9ce00516`. These are local patches, not an upstream
merge, a relay deployment or a universal reachability/security guarantee.

## Exact stack

Apply these three patches in order. Their existing numbers and bytes are retained.

- **`0003-client-relay-response-bound.patch`** — bounds client relay-response
  collection before allocation.
  SHA-256: `6c55a783245e3045d9cf852bfa5836fb0b15528542da6d903c0b3f14f038d04d`
- **`0004-cancellable-peer-attempt.patch`** — caller-owned connection attempts,
  atomic abort classification and access to retained accepted connections.
  SHA-256: `f68612aed60d8df84ceeb3bba85a14b06cc145d1596567cb335d3a4183217dba`
- **`0005-peer-handshake-hook.patch`** — constructor-installed ordinary/virtual
  handshake hooks for application-owned admission and refusal.
  SHA-256: `5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386`

Existing constructors/default dispatch and wire formats remain unchanged. Route
selection, scoped grants, admission budgets and ownership policy live in our core.
Relay forwarding and socket construction use upstream's existing implementation;
the application does not operate a custom or public relay service.

## Preparation and checks

```sh
bash scripts/test-native.sh
```

Run from the repository root after fetching locked dependencies. `prepare-native.sh`
archives the exact cached revision (or `SAORSA_CHECKOUT`), verifies patch hashes and
rejects symlinked patch files, checks/applies the patches, and copies the workspace.
Only the generated manifest receives a local dependency override. Only the pinned
Saorsa source field is removed from the copied lock; versions/checksums are preserved.
Builds are locked/offline and never modify Cargo's upstream cache.

Prepared sources default to `target/patched-source/`; host output to `target/build/`.
Each prepared workspace records the preparation recipe and patch hashes in
`preparation.json`. Reusing `THEPEER_PREPARED_WORKSPACE` checks that receipt, current
workspace sources, manifest and lock. A missing or stale receipt requires fresh
preparation; do not copy receipts into older trees. The receipt tracks preparation
inputs, not subsequent manual modifications to the generated Saorsa tree. Reuse
only trusted, unmodified prepared sources.

This is repeatable source preparation, not bit-for-bit cross-toolchain/platform
binary reproducibility. Reuse build output directories rather than deleting caches.

Android builds select `tunnel-android` and require a fresh `saorsa-routing-v2`
source/artifact receipt covering these three patches. Build/install/device validation
remains operator work. See [handoff](../../docs/HUMAN-TESTING.md).

## API limits

0003 caps CONNECT-UDP response bodies at 4096 bytes before allocation. It is not a
bound on all transport memory, tables or stream traffic.

0004 allows no-response attempts to be explicitly aborted; it does not turn TLS or
protocol errors into safe fallback. 0005 lets the application refuse unsolicited
work from endpoint construction, including virtual ingress and coordinator-requested
outgoing handshakes. Default behavior is unchanged for callers without a handler.

The application preserves actual retained-connection pin verification before grants,
owned cleanup before fallback, and sticky Stop. Configured connection limits are not
proof of a global resource cap. Relay withdrawal is not immediate remote reservation
or FD reclamation (upstream ADR-011).

See the [current-scope audit](../../docs/research/current-scope-patch-audit.md) for
call-site evidence and reduced-stack validation, and
[upstream divergence](../../docs/research/saorsa-upstream-divergence.md) for the
application/transport boundary. Older research describes historical patch stacks;
it is not a preparation recipe. Current release gates are in
[release readiness](../../docs/RELEASE-READINESS.md).

## Provenance and notices

The patches contain upstream context bearing **Copyright 2024 Saorsa Labs Ltd.**
The source offers MIT or Apache-2.0; upstream portions here use the MIT option.
[UPSTREAM-LICENSE-MIT](UPSTREAM-LICENSE-MIT), including
**Copyright (c) 2018 The quinn Developers**, is retained verbatim, as are original
source headers when applying the patches.

Original additions use the repository's [MIT license](../../LICENSE).
Upstream portions retain the licenses and notices described above.
