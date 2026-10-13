# Reviewed Saorsa transport patch stack

Base: **0.37.0**, `WithAutonomi/saorsa-transport` revision
`e2f08388088313c82cac393afa6ca27b9ce00516`. These are local patches, not an upstream
merge, a relay deployment or a universal reachability/security guarantee.

## Exact stack

Apply in order. Cleanup removed duplicate experiment copies, not these patches.

| Patch | Purpose | SHA-256 |
| --- | --- | --- |
| `0001-stream-forwarding-policy.patch` | Opt-in forwarding policy/limits and local allocator | `761cb44c54c20ce852ac416f41a91fbbf2a70553e2f69515494727423e4b5ae3` |
| `0002-bounded-stream-socket.patch` | Bounded established-stream socket and lifecycle controls | `53a792b581ac9dc6865f8c1b392452208f270e72a763729a603b9bbc7dc54e73` |
| `0003-client-relay-response-bound.patch` | Bounded client relay-response collection | `6c55a783245e3045d9cf852bfa5836fb0b15528542da6d903c0b3f14f038d04d` |
| `0004-cancellable-peer-attempt.patch` | Cancellable connection attempts and retained accepted connections | `f68612aed60d8df84ceeb3bba85a14b06cc145d1596567cb335d3a4183217dba` |
| `0005-peer-handshake-hook.patch` | Opt-in constructor-installed ordinary/virtual handshake hooks | `5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386` |

Existing constructors/default dispatch and wire formats remain unchanged. Route
selection, scoped grants, admission budgets and ownership policy live in our core.
The full reviewed stack is deliberately preserved; removing opt-in APIs that the
retired experiments exercised would be a separate transport change/review.

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
Reuse an existing target directory rather than deleting caches. This is repeatable
source preparation, not bit-for-bit cross-toolchain/platform binary reproducibility.

Android builds select `tunnel-android` and require a fresh `saorsa-routing-v2`
source/artifact receipt covering all five patches. Build/install/device validation
remains operator work. See [handoff](../../docs/HUMAN-TESTING.md).

## API limits

0001's policy receives frame metadata before allocation, send metadata before UDP
transmission and receive metadata before queueing. It must atomically authorize
known sessions and reserve budgets. Denied receive packets are dropped; frame/send
denials terminate the session. Callbacks must be non-blocking/non-panicking.
Asynchronous pacing waits for credit and must observe cancellation/revocation.
It is an opt-in seam, **not** protection for every upstream forwarding entry point.
The loopback-only allocator disables UPnP but does not itself authorize forwarding.
The removed relay-service prototype is not needed to build or run the MVP.

0002's established-stream socket accepts explicit queue/frame bounds and exposes
status/close/shutdown controls. Its caller must authenticate and retain the carrier
and complete admission first. These limits are not a bound on all process memory;
other upstream control tables and partial-frame behavior have separate limitations.

0004 allows no-response attempts to be explicitly aborted; it does not turn TLS or
protocol errors into safe fallback. 0005 lets the application refuse unsolicited
work from endpoint construction, including virtual ingress. Its default behavior
is unchanged for callers that do not install a handler.

The application preserves actual retained-connection pin verification before grants,
owned cleanup before fallback, and sticky Stop. Configured connection limits are not
proof of a global resource cap. Relay withdrawal is not immediate remote reservation
or FD reclamation (ADR-011). No public relay service is operated by our clients.

Review rationale and dated evidence remain in
[upstream divergence](../../docs/research/saorsa-upstream-divergence.md) and related
research; old experiment paths in those records are historical, not current commands.
Current test evidence is in [release readiness](../../docs/RELEASE-READINESS.md).

## Provenance and notices

The patches contain upstream context bearing **Copyright 2024 Saorsa Labs Ltd.**
The source offers MIT or Apache-2.0; upstream portions here use the MIT option.
[UPSTREAM-LICENSE-MIT](UPSTREAM-LICENSE-MIT), including
**Copyright (c) 2018 The quinn Developers**, is retained verbatim, as are original
source headers when applying the patches.

Original additions use the repository's [MIT license](../../LICENSE).
Upstream portions retain the licenses and notices described above.
